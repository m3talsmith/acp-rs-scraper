//! Optional progress hook for a long key derivation.
//!
//! The core crate does not draw a terminal. The application installs a hook on
//! the thread that runs Argon2id, and that hook updates the progress screen.

use std::cell::Cell;

thread_local! {
    static HOOK: Cell<Option<fn(&str)>> = const { Cell::new(None) };
}

/// Install `hook` on this thread. A later derivation calls it with a stage name.
///
/// The hook is not process-global. A worker thread that derives a key installs
/// its own hook. An unset hook does nothing.
pub fn set_stage_hook(hook: fn(&str)) {
    HOOK.with(|slot| slot.set(Some(hook)));
}

/// Tell the installed hook which step is running.
pub(crate) fn note_stage(stage: &str) {
    HOOK.with(|slot| {
        if let Some(hook) = slot.get() {
            hook(stage);
        }
    });
}
