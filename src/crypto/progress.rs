//! A spinner for a long encryption, backup, or restore step.
//!
//! The slow work runs on a background thread. When stdout is a terminal this
//! module redraws a spinner, the current stage, and the elapsed time, and it
//! drops keypresses so a stray key cannot cancel a half-finished copy. A pipe
//! or a non-interactive run calls the work on this thread and prints nothing.

use anyhow::{Result, bail};
#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::io::{self, IsTerminal, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

thread_local! {
    static STAGE_TX: RefCell<Option<Sender<String>>> = const { RefCell::new(None) };
    #[cfg(test)]
    static RECORD: Cell<bool> = const { Cell::new(false) };
    #[cfg(test)]
    static STAGES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Point the core crate's derivation hook at [`set_stage`] on this thread.
fn install_stage_hook() {
    acp_rs_node_core::stage::set_stage_hook(set_stage);
}

/// Name the step that is running. No effect unless [`run_busy`] installed a
/// reporter on this thread.
pub(crate) fn set_stage(stage: &str) {
    #[cfg(test)]
    if RECORD.with(|on| on.get()) {
        STAGES.with(|slot| slot.borrow_mut().push(stage.to_string()));
    }
    STAGE_TX.with(|slot| {
        if let Some(tx) = slot.borrow().as_ref() {
            let _ = tx.send(stage.to_string());
        }
    });
}

/// Run `work` while a terminal shows `title`, a spinner, the stage, and the
/// elapsed time.
///
/// Keypresses are discarded. The work is not cancelled from the keyboard:
/// stopping a copy midway would leave the database half-moved. When stdout is
/// not a terminal the work runs on this thread and nothing is drawn.
pub(crate) fn run_busy<T, F>(title: &str, work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    if !io::stdout().is_terminal() {
        install_stage_hook();
        return work();
    }
    let (stage_tx, stage_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        install_stage_hook();
        STAGE_TX.with(|slot| *slot.borrow_mut() = Some(stage_tx));
        let outcome = catch_unwind(AssertUnwindSafe(work));
        STAGE_TX.with(|slot| *slot.borrow_mut() = None);
        let _ = done_tx.send(outcome);
    });
    let _guard = TermGuard::enter().ok();
    let started = Instant::now();
    let mut stage = String::from("Starting");
    let mut frame = 0usize;
    loop {
        while let Ok(next) = stage_rx.try_recv() {
            stage = next;
        }
        if _guard.is_some() {
            let _ = paint(title, &stage, started.elapsed(), frame);
            drain_keys();
        }
        frame = frame.wrapping_add(1);
        match done_rx.recv_timeout(Duration::from_millis(80)) {
            Ok(Ok(result)) => return result,
            Ok(Err(_)) => bail!("the operation panicked"),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                bail!("the operation stopped before it finished")
            }
        }
    }
}

/// Start collecting [`set_stage`] names on this thread.
#[cfg(test)]
pub(crate) fn record_stages() {
    install_stage_hook();
    RECORD.with(|on| on.set(true));
    STAGES.with(|slot| slot.borrow_mut().clear());
}

/// Stages recorded since [`record_stages`].
#[cfg(test)]
pub(crate) fn stages() -> Vec<String> {
    STAGES.with(|slot| slot.borrow().clone())
}

struct TermGuard;

impl TermGuard {
    fn enter() -> io::Result<Self> {
        crate::cli::apply_color_mode(crate::cli::current_color_mode());
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::cursor::Hide
        )?;
        Ok(Self)
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::cursor::Show,
            crossterm::terminal::LeaveAlternateScreen
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

fn paint(title: &str, stage: &str, elapsed: Duration, frame: usize) -> io::Result<()> {
    use crossterm::{
        cursor::MoveTo,
        style::{Attribute, Print, SetAttribute},
        terminal::{Clear, ClearType},
    };
    let (_cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let row = rows.saturating_sub(8) / 2;
    let spinner = FRAMES[frame % FRAMES.len()];
    crossterm::execute!(
        io::stdout(),
        Clear(ClearType::All),
        MoveTo(2, row),
        SetAttribute(Attribute::Bold),
        Print(title),
        SetAttribute(Attribute::Reset),
        MoveTo(4, row + 2),
        Print(format!("{spinner}  {stage}")),
        MoveTo(4, row + 3),
        Print(format_elapsed(elapsed)),
        MoveTo(2, row + 5),
        Print("Keys are ignored until this finishes."),
    )?;
    io::stdout().flush()
}

fn format_elapsed(elapsed: Duration) -> String {
    let total = elapsed.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn drain_keys() {
    while crossterm::event::poll(Duration::ZERO).unwrap_or(false) {
        let _ = crossterm::event::read();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_terminal_the_work_runs_inline() {
        if io::stdout().is_terminal() {
            return;
        }
        let value = run_busy("Working", || Ok::<_, anyhow::Error>(7)).unwrap();
        assert_eq!(value, 7);
        let err = run_busy("Working", || -> Result<()> { bail!("nope") }).unwrap_err();
        assert!(err.to_string().contains("nope"), "{err}");
    }

    #[test]
    fn set_stage_is_harmless_without_a_reporter() {
        set_stage("Deriving key");
    }

    #[test]
    fn elapsed_time_is_minutes_and_seconds() {
        assert_eq!(format_elapsed(Duration::from_secs(4)), "0:04");
        assert_eq!(format_elapsed(Duration::from_secs(65)), "1:05");
        assert_eq!(format_elapsed(Duration::from_secs(3661)), "1:01:01");
    }
}
