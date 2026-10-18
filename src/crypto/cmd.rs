//! `--identity-init`, `--acp-id`, `--shares-print` and `--shares-verify`.
//! The phrase and the shares are printed once, to the terminal, and are never logged.

use anyhow::{Result, bail};
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

use super::master::{self, Master};
use super::password::{check_password, default_params};
use super::pin::Pin;
use super::slip39;
use crate::cli::prompt;
use crate::config::{self, home};

const YEAR_SECS: u64 = 365 * 24 * 60 * 60;

/// Create a master, show the 24 words, and pin the `acp-id`.
pub fn identity_init() -> Result<()> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!(
            "--identity-init needs a terminal, so the recovery phrase is not written to a log or a pipe. \
             There is no password reset."
        );
    }
    let home = home()?;
    if Pin::load(&home)?.is_some() {
        bail!(
            "an acp-id is already pinned. A second identity would be a different account, so this does nothing."
        );
    }
    let password = prompt_new_password()?;
    let entropy = Zeroizing::new(master::new_entropy());
    let phrase = Zeroizing::new(master::phrase_from_entropy(&entropy)?);
    let master = master::derive_master(&phrase, password.as_str(), default_params())?;
    println!();
    println!("Write these 24 words down. They are not stored, not logged, and not copied for you.");
    println!("There is no password reset. The password is not part of the phrase.");
    println!();
    println!("    {}", phrase.as_str());
    println!();
    println!("Account id: {}", master.acp_id);
    println!();
    let typed = prompt("Type the 24 words to confirm, then Enter: ")?;
    if master::normalize_phrase(&typed) != master::normalize_phrase(&phrase) {
        bail!("the phrase did not match. Nothing was saved. Run --identity-init again.");
    }
    store_pin(&home, &master)?;
    println!();
    println!("Pinned {}.", master.acp_id);
    println!("The master secret was not written to disk.");
    println!(
        "There is no password reset. A lost password together with lost shares cannot be recovered."
    );
    println!();
    let answer = prompt("Print the 5 recovery shares now? The phrase cannot be postponed. [y/N] ")?;
    if is_yes(&answer) {
        print_sheets(master.acp_id.as_str(), &entropy)?;
        clear_shares_pending(&home)?;
        note_shares_checked(&home)?;
    } else {
        mark_shares_pending(&home)?;
        println!(
            "Shares postponed. Run --shares-print on a terminal when the sheets can be written down."
        );
    }
    Ok(())
}

/// Print the pinned `acp-id`.
pub fn print_acp_id() -> Result<()> {
    let home = home()?;
    match Pin::load(&home)? {
        Some(p) => {
            println!("{}", p.acp_id);
            Ok(())
        }
        None => bail!("no acp-id is pinned yet. Run --identity-init on a terminal."),
    }
}

fn store_pin(home: &std::path::Path, master: &Master) -> Result<()> {
    let pin = Pin {
        acp_id: master.acp_id.clone(),
        argon2: default_params(),
        sign_pub: master.sign_pub,
        wrap_pub: master.wrap_pub,
    };
    pin.store(home)
}

fn prompt_new_password() -> Result<Zeroizing<String>> {
    let first = Zeroizing::new(rpassword::prompt_password(
        "Password (at least 12 characters): ",
    )?);
    check_password(first.as_str())?;
    let second = Zeroizing::new(rpassword::prompt_password("Repeat password: ")?);
    if first.as_str() != second.as_str() {
        bail!("the passwords did not match");
    }
    Ok(first)
}

/// Print five new sheets for the pinned account. Needs the phrase or three existing shares,
/// plus the password. The password is not written into the shares.
pub fn shares_print(file: Option<&Path>) -> Result<()> {
    let (home, seed) = unlock_seed(file, "--shares-print")?;
    print_sheets_for_home(&home, &seed)?;
    Ok(())
}

/// Check three to five shares against the pinned `acp-id`. The seed is not stored.
pub fn shares_verify(file: Option<&Path>) -> Result<()> {
    let (home, seed) = unlock_seed(file, "--shares-verify")?;
    drop(seed);
    clear_shares_pending(&home)?;
    note_shares_checked(&home)?;
    println!("These shares recover the pinned account.");
    println!("The seed was not written. Run --shares-verify once a year.");
    println!(
        "Reissuing shares of this seed does not invalidate older shares. Only a master rotation does."
    );
    Ok(())
}

/// Write an encrypted backup. Needs a terminal so the password is not taken from a pipe.
pub fn backup_now(output: Option<&Path>) -> Result<()> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!("--backup needs a terminal, so the password is not read from a pipe");
    }
    let home = home()?;
    let password = Zeroizing::new(rpassword::prompt_password("Password: ")?);
    check_password(password.as_str())?;
    let params = Pin::load(&home)?
        .map(|p| p.argon2)
        .unwrap_or_else(default_params);
    let dest = match output {
        Some(path) => path.to_path_buf(),
        None => match super::backup::load_schedule(&home)? {
            Some(schedule) => {
                let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                Path::new(&schedule.dest).join(format!("backup-{stamp}.acpbackup"))
            }
            None => super::backup::default_backup_path(&home),
        },
    };
    super::backup::write_backup(&home, &dest, password.as_str(), params)?;
    if let Some(schedule) = super::backup::load_schedule(&home)? {
        super::backup::mark_backup_ran(&home)?;
        super::backup::prune_backups(Path::new(&schedule.dest), schedule.keep)?;
    }
    println!("Wrote {}", dest.display());
    println!("session.json and the node secret were not included.");
    Ok(())
}

/// Restore a backup into this home.
pub fn restore_now(path: &Path) -> Result<()> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!("--restore needs a terminal, so the password is not read from a pipe");
    }
    let home = home()?;
    let header = super::backup::inspect(path)?;
    let material = if super::backup::needs_phrase(&home, &header)? {
        Some(read_block(
            "Paste the 24-word phrase, or at least 3 shares, then a blank line:",
        )?)
    } else {
        None
    };
    let password = Zeroizing::new(rpassword::prompt_password("Password: ")?);
    super::backup::restore_backup(&home, path, password.as_str(), material.as_deref())?;
    println!("Restored the archive into this home.");
    println!(
        "Sign in again. The node secret was not in the backup, so this machine is not a clone of the old node."
    );
    Ok(())
}

/// Store a backup schedule. A later headless service runs it; this build records it and reminds you.
pub fn backup_schedule(cadence: &str, keep: u32, dest: &Path) -> Result<()> {
    let home = home()?;
    super::backup::save_schedule(&home, cadence, keep, dest)?;
    println!(
        "Backup schedule: {cadence}, keep {keep}, destination {}. Run --backup to write one now.",
        dest.display()
    );
    Ok(())
}

/// Status line for the terminal UI, when shares were postponed or last checked over a year ago.
pub fn shares_reminder(home: &Path) -> Option<String> {
    if home.join(config::SHARES_PENDING_REL_PATH).is_file() {
        return Some(
            "Recovery shares are not printed yet. Run --shares-print. A lost password and lost shares cannot be recovered."
                .into(),
        );
    }
    let path = home.join(config::SHARES_VERIFIED_REL_PATH);
    let text = std::fs::read_to_string(&path).ok()?;
    let stamp: u64 = text.trim().parse().ok()?;
    let now = now_secs();
    if now.saturating_sub(stamp) > YEAR_SECS {
        Some("Recovery shares have not been checked in a year. Run --shares-verify.".into())
    } else {
        None
    }
}

/// Recover a 256-bit seed from a 24-word phrase or from at least three SLIP-39 shares.
pub fn seed_from_material(text: &str) -> Result<Zeroizing<[u8; 32]>> {
    master::seed_from_material(text)
}

fn unlock_seed(
    file: Option<&Path>,
    flag: &str,
) -> Result<(std::path::PathBuf, Zeroizing<[u8; 32]>)> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!("{flag} needs a terminal, so the phrase and the password are not read from a pipe");
    }
    let home = home()?;
    let pin = Pin::load(&home)?.ok_or_else(|| {
        anyhow::anyhow!("no acp-id is pinned yet. Run --identity-init on a terminal.")
    })?;
    let text = match file {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?,
        None => read_block("Paste the 24-word phrase, or at least 3 shares, then a blank line:")?,
    };
    let seed = seed_from_material(&text)?;
    let phrase = Zeroizing::new(master::phrase_from_entropy(&seed)?);
    let password = Zeroizing::new(rpassword::prompt_password("Password: ")?);
    let master = master::derive_master(phrase.as_str(), password.as_str(), pin.argon2)?;
    master.require_id(&pin.acp_id)?;
    Ok((home, seed))
}

fn print_sheets_for_home(home: &Path, seed: &[u8; 32]) -> Result<()> {
    let pin = Pin::load(home)?.ok_or_else(|| anyhow::anyhow!("no acp-id is pinned"))?;
    print_sheets(pin.acp_id.as_str(), seed)?;
    clear_shares_pending(home)?;
    note_shares_checked(home)?;
    Ok(())
}

fn print_sheets(acp_id: &str, seed: &[u8; 32]) -> Result<()> {
    let shares = slip39::split_seed(seed)?;
    println!();
    println!("Five recovery shares, any 3 of which restore the seed. The password is not in them.");
    println!(
        "Reissuing shares of this seed does not invalidate older shares. Only a master rotation does."
    );
    println!("Write each sheet down separately. They are not stored and not copied for you.");
    println!("Run --shares-verify once a year.");
    println!();
    for mnemonic in &shares {
        println!("{}", slip39::sheet(acp_id, mnemonic)?);
    }
    Ok(())
}

fn read_block(msg: &str) -> Result<String> {
    eprintln!("{msg}");
    let stdin = std::io::stdin();
    let mut lines = Vec::new();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            if !lines.is_empty() {
                break;
            }
            continue;
        }
        lines.push(line);
    }
    if lines.is_empty() {
        bail!("nothing was entered");
    }
    Ok(lines.join("\n"))
}

fn is_yes(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

pub(crate) fn mark_shares_pending(home: &Path) -> Result<()> {
    write_marker(
        home,
        config::SHARES_PENDING_REL_PATH,
        "shares not printed yet\n",
    )
}

pub(crate) fn clear_shares_pending(home: &Path) -> Result<()> {
    let path = home.join(config::SHARES_PENDING_REL_PATH);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

fn note_shares_checked(home: &Path) -> Result<()> {
    write_marker(
        home,
        config::SHARES_VERIFIED_REL_PATH,
        &format!("{}\n", now_secs()),
    )
}

fn write_marker(home: &Path, rel: &str, body: &str) -> Result<()> {
    let path = home.join(rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;
    f.write_all(body.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
