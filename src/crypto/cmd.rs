//! `--identity-init`, `--acp-id`, `--show-phrase`, `--shares-print` and `--shares-verify`.
//! The phrase and the shares are printed to the terminal and are not logged.
//! `--identity-init` also stores an encrypted copy of the phrase entropy so
//! `--show-phrase` can show the words again after the password is entered.

use anyhow::{Result, bail};
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

use super::master::{self, Master};
use super::password::{MIN_PASSWORD_CHARS, check_password, default_params};
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
    clear_screen();
    println!("Write these 24 words on paper. A screenshot or a photo is not a safe copy.");
    println!("There is no password reset. The password is not part of the phrase.");
    println!("This screen is the only place the words are shown. They are not logged.");
    println!();
    println!("    {}", phrase.as_str());
    println!();
    println!("Account id: {}", master.acp_id);
    println!();
    let typed = prompt("Type the 24 words to confirm, then Enter: ")?;
    if master::normalize_phrase(&typed) != master::normalize_phrase(&phrase) {
        bail!("the phrase did not match. Nothing was saved. Run --identity-init again.");
    }
    let home_seal = home.clone();
    let password_seal = Zeroizing::new(password.as_str().to_string());
    let entropy_seal = entropy.clone();
    super::progress::run_busy("Saving the recovery phrase", move || {
        super::recovery::seal(
            &home_seal,
            password_seal.as_str(),
            default_params(),
            &entropy_seal,
        )
    })?;
    if let Err(err) = store_pin(&home, &master) {
        let _ = std::fs::remove_file(super::recovery::seal_path(&home));
        return Err(err);
    }
    let db = home.join(crate::config::DB_REL_PATH);
    if db.is_file() {
        super::atrest::add_master_recipient(&db, password.as_str(), &master)?;
    }
    super::recovery::clear_next(&home)?;
    println!();
    println!("Pinned {}.", master.acp_id);
    println!(
        "An encrypted copy of these words is at ~/.acp/recovery.seal. --show-recovery shows them again after you enter the password."
    );
    println!(
        "That file is not a paper copy. It does not help if the password is lost, and a new computer still needs the words or 3 shares."
    );
    println!("The words were not written to a log.");
    println!(
        "There is no password reset. A lost password together with lost shares cannot be recovered."
    );
    println!();
    println!(
        "The shares come next, one at a time. Any {} of {} rebuild the seed. One group, group threshold {} of {}.",
        slip39::MEMBER_THRESHOLD,
        slip39::MEMBER_COUNT,
        slip39::GROUP_THRESHOLD,
        slip39::GROUP_COUNT
    );
    if let Err(err) = present_shares(master.acp_id.as_str(), &entropy, false) {
        mark_shares_pending(&home)?;
        return Err(err);
    }
    clear_shares_pending(&home)?;
    note_shares_checked(&home)?;
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

/// Create this machine's node key, certify it with the master, and drop the master.
///
/// The phrase is read from the seal when that file exists. Otherwise it is typed
/// on a terminal and is not printed back. The node secret is not printed.
pub fn node_init(label: Option<&str>) -> Result<()> {
    let home = home()?;
    if Pin::load(&home)?.is_none() {
        bail!("no acp-id is pinned yet. Run --identity-init on a terminal.");
    }
    let password = password_for_db(true)?;
    let entropy = if super::recovery::seal_exists(&home) {
        super::recovery::open(&home, password.as_str())?
    } else {
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            bail!(
                "--node-init needs the recovery phrase, and this computer has no sealed copy. Run it on a terminal."
            );
        }
        let typed = Zeroizing::new(read_block(
            "Paste the 24-word phrase, or at least 3 shares, then a blank line:",
        )?);
        master::seed_from_material(typed.as_str())?
    };
    let label = match label {
        Some(label) => label.to_string(),
        None => super::node_store::default_label(),
    };
    let public = super::node_store::create_first_node(&home, password.as_str(), &label, &entropy)?;
    println!("Node: {}", public.label);
    println!("Account: {}", public.acp_id);
    println!("Peer id: {}", public.peer_id);
    println!("Fingerprint: {}", public.fingerprint);
    println!("Verification: {}", public.verification_words()?);
    println!(
        "Node certificate expires at {} (unix time). Authorizer certificate expires at {}.",
        public.node_expires, public.authorizer_expires
    );
    println!("The node secret was not printed. It is not in a backup.");
    Ok(())
}

/// Print the public node record and whether its certificate still checks out.
pub fn print_node_id() -> Result<()> {
    let home = home()?;
    let Some(public) = super::node_store::PublicNode::load(&home)? else {
        bail!("no node on this computer yet. Run --node-init after --identity-init.");
    };
    println!("Node: {}", public.label);
    println!("Account: {}", public.acp_id);
    println!("Peer id: {}", public.peer_id);
    println!("Fingerprint: {}", public.fingerprint);
    println!("Verification: {}", public.verification_words()?);
    println!("Node certificate expires at {}.", public.node_expires);
    match Pin::load(&home)? {
        Some(pin) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            match acp_red_star_node_core::node::verify_chain(
                &public.node,
                Some(&public.authorizer),
                &pin.sign_pub,
                now,
                None,
            ) {
                Ok(()) => println!("Certificate: valid"),
                Err(err) => println!("Certificate: {err}"),
            }
        }
        None => println!("Certificate: no pinned acp-id to check it against"),
    }
    Ok(())
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
    let first = Zeroizing::new(rpassword::prompt_password(format!(
        "Password (at least {MIN_PASSWORD_CHARS} characters): "
    ))?);
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

/// Unlock or create the database for a command that is about to use it.
/// A 2.x plaintext database is refused with the `--encrypt-db` instructions.
/// Commands that do not touch the database do not call this.
///
/// On a terminal, a database that was just created also explains the recovery
/// phrase. A pipe prints nothing, so a later terminal session can still show it.
pub fn prepare_db() -> Result<()> {
    prepare_db_quiet()?;
    if std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
    {
        offer_on_terminal(&home()?)?;
    }
    Ok(())
}

/// Open or create the database without printing the recovery steps.
/// The terminal UI calls this, then shows those steps itself.
pub fn prepare_db_quiet() -> Result<()> {
    prepare_path(&home()?.join(crate::config::DB_REL_PATH))
}

/// Unlock or create `db`. A key already cached for this process is reused, so a
/// job started from the terminal UI does not prompt and does not read `ACP_PASSWORD`.
pub(crate) fn prepare_path(db: &Path) -> Result<()> {
    if let Some(msg) = super::atrest::legacy_plaintext_notice(db) {
        bail!("{msg}");
    }
    if super::atrest::is_unlocked(db) {
        return super::atrest::prepare(db, None);
    }
    let existing = db.is_file();
    let password = password_for_db(existing)?;
    let db = db.to_path_buf();
    let title = if existing {
        "Opening the database"
    } else {
        "Creating the encrypted database"
    };
    super::progress::run_busy(title, move || {
        super::atrest::prepare(&db, Some(password.as_str()))
    })
}

/// Encrypt a plaintext database in place, after a backup.
pub fn encrypt_db() -> Result<()> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::env::var("ACP_PASSWORD")
            .ok()
            .filter(|p| !p.is_empty())
            .is_none()
    {
        bail!("--encrypt-db needs a terminal, or ACP_PASSWORD for a non-interactive run");
    }
    let home = home()?;
    let db = home.join(crate::config::DB_REL_PATH);
    let password = password_for_db(true)?;
    let dest = super::backup::default_backup_path(&home);
    let db_shown = db.display().to_string();
    let dest_shown = dest.display().to_string();
    super::progress::run_busy("Encrypting the database", move || {
        super::atrest::encrypt_db(&db, password.as_str(), &dest)
    })?;
    super::recovery::mark_next(&home)?;
    println!("Encrypted {db_shown}. A backup was written to {dest_shown}.");
    println!("The plaintext file was removed after the encrypted copy matched.");
    println!(
        "2.x cannot read this database or that backup. A copy of the plaintext file saved before this command is what a 2.x binary can open."
    );
    if std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
    {
        explain_recovery(&home)?;
    }
    Ok(())
}

/// Change the password that wraps the database key and the account key.
/// The account id stays the same. Needs a terminal.
pub fn change_password() -> Result<()> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!("--change-password needs a terminal. There is no password reset.");
    }
    let home = home()?;
    let db = home.join(crate::config::DB_REL_PATH);
    if !super::atrest::bootstrap_path(&db).is_file() {
        bail!(
            "there is no encrypted database here. A plaintext 2.x database is migrated with --encrypt-db."
        );
    }
    let current = Zeroizing::new(rpassword::prompt_password("Current password: ")?);
    let next = prompt_new_password()?;
    let params = super::atrest::password_params(&db)?;
    let phrase_follows = super::progress::run_busy("Changing the password", move || {
        super::atrest::rewrap_local(&db, current.as_str(), next.as_str(), params)
    })?;
    println!("The database password was changed.");
    println!(
        "The account id did not change. A backup written with the old password still needs that password."
    );
    if super::recovery::seal_exists(&home) {
        if phrase_follows {
            println!("The recovery phrase now uses the new password. --show-phrase asks for it.");
        } else {
            println!(
                "The recovery phrase is still sealed with the previous password. --show-phrase needs that password."
            );
        }
    }
    Ok(())
}

fn password_for_db(existing: bool) -> Result<Zeroizing<String>> {
    if let Ok(value) = std::env::var("ACP_PASSWORD") {
        if !value.is_empty() {
            let password = Zeroizing::new(value);
            check_password(password.as_str())?;
            return Ok(password);
        }
    }
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!("this database needs a password. Set ACP_PASSWORD, or run from a terminal.");
    }
    if existing {
        let password = Zeroizing::new(rpassword::prompt_password("Password: ")?);
        check_password(password.as_str())?;
        return Ok(password);
    }
    prompt_new_password()
}

/// Write an encrypted backup. Needs a terminal so the password is not taken from a pipe.
pub fn backup_now(output: Option<&Path>) -> Result<()> {
    let home = home()?;
    let password = password_for_db(true)?;
    let output = output.map(|path| path.to_path_buf());
    let dest = super::progress::run_busy("Writing the backup", move || {
        let db = home.join(crate::config::DB_REL_PATH);
        let stretch = if super::atrest::bootstrap_path(&db).is_file() {
            super::atrest::unlock_stretch(&db, password.as_str())?
        } else {
            let params = Pin::load(&home)?
                .map(|p| p.argon2)
                .unwrap_or_else(default_params);
            super::password::Stretch::fresh(password.as_str(), params)?
        };
        let dest = match output {
            Some(path) => path,
            None => match super::backup::load_schedule(&home)? {
                Some(schedule) => {
                    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                    Path::new(&schedule.dest).join(format!("backup-{stamp}.acpbackup"))
                }
                None => super::backup::default_backup_path(&home),
            },
        };
        super::backup::write_backup_with(&home, &dest, &stretch)?;
        if let Some(schedule) = super::backup::load_schedule(&home)? {
            super::backup::mark_backup_ran(&home)?;
            super::backup::prune_backups(Path::new(&schedule.dest), schedule.keep)?;
        }
        Ok(dest)
    })?;
    println!("Wrote {}", dest.display());
    println!("session.json and the node secret were not included.");
    Ok(())
}

/// Restore a backup into this home.
pub fn restore_now(path: &Path) -> Result<()> {
    let home = home()?;
    let header = super::backup::inspect(path)?;
    let material = if super::backup::needs_phrase(&home, &header)? {
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            bail!("--restore on a new machine needs a terminal for the phrase or the shares");
        }
        Some(read_block(
            "Paste the 24-word phrase, or at least 3 shares, then a blank line:",
        )?)
    } else {
        None
    };
    let password = password_for_db(true)?;
    let path = path.to_path_buf();
    super::progress::run_busy("Restoring the backup", move || {
        super::backup::restore_backup(&home, &path, password.as_str(), material.as_deref())
    })?;
    println!("Restored the archive into this home.");
    println!(
        "Sign in again. The node secret was not in the backup, so this machine is not a clone of the old node."
    );
    Ok(())
}

/// Store a backup schedule. A later headless service runs it; this build records it and reminds you.
pub fn backup_schedule(cadence: &str, keep: u32, dest: &Path) -> Result<()> {
    prepare_db()?;
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
            "Recovery shares are not printed yet. Run --show-recovery. A lost password and lost shares cannot be recovered."
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
    present_shares_with(acp_id, &shares, true, &mut std::io::stdout(), read_stdin_line)
}

fn clear_screen() {
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        let _ = super::recovery::clear_terminal();
    }
}

fn read_stdin_line() -> Result<String> {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

/// Show each share on its own screen. The next share is printed only after this
/// one is typed back. Nothing here is logged or written to disk.
pub fn present_shares_with(
    acp_id: &str,
    shares: &[String],
    fresh: bool,
    out: &mut dyn Write,
    mut read_line: impl FnMut() -> Result<String>,
) -> Result<()> {
    let count = slip39::MEMBER_COUNT;
    let threshold = slip39::MEMBER_THRESHOLD;
    let groups = slip39::GROUP_COUNT;
    let group_need = slip39::GROUP_THRESHOLD;
    writeln!(out)?;
    writeln!(
        out,
        "Any {threshold} of {count} shares rebuild the seed. One group, group threshold {group_need} of {groups}."
    )?;
    writeln!(out, "The password is not in a share. A share is not logged and is not stored.")?;
    if fresh {
        writeln!(out, "This is a new split of the same master secret.")?;
        writeln!(
            out,
            "A split uses new randomness, so an older set cannot be printed again."
        )?;
        writeln!(
            out,
            "Destroy any older shares of this secret. An older share still rebuilds it until that paper is destroyed."
        )?;
    }
    writeln!(out, "Write each share on its own paper. Run --shares-verify once a year.")?;
    out.flush()?;
    for (i, mnemonic) in shares.iter().enumerate() {
        let n = i + 1;
        clear_screen();
        writeln!(out)?;
        writeln!(
            out,
            "Share {n} of {count}. Threshold {threshold} of {count}. Group threshold {group_need} of {groups}."
        )?;
        writeln!(out, "{}", slip39::sheet(acp_id, mnemonic)?)?;
        writeln!(out, "Type this share to confirm you wrote it down, then Enter.")?;
        out.flush()?;
        loop {
            let typed = read_line()?;
            if master::normalize_phrase(&typed) == master::normalize_phrase(mnemonic) {
                writeln!(out, "Share {n} of {count} is confirmed.")?;
                out.flush()?;
                break;
            }
            writeln!(out, "That share did not match. It is still on screen. Type it again.")?;
            out.flush()?;
        }
    }
    clear_screen();
    writeln!(out, "All {count} shares were confirmed. They were not written to disk.")?;
    out.flush()?;
    Ok(())
}

fn present_shares(acp_id: &str, seed: &[u8; 32], fresh: bool) -> Result<()> {
    let shares = slip39::split_seed(seed)?;
    present_shares_with(acp_id, &shares, fresh, &mut std::io::stdout(), read_stdin_line)
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

/// What the next terminal session should do about the phrase and the shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryNext {
    /// Nothing is waiting.
    Quiet,
    /// No account yet. Create the phrase and the shares.
    Create,
    /// The seal can open the phrase. Show it, then a fresh split.
    ShowAgain,
    /// A pin exists and the seal does not. The password cannot rebuild the words.
    CannotRebuild,
}

/// Decide from the home directory. This does not print the phrase or a share.
pub fn recovery_next(home: &Path) -> Result<RecoveryNext> {
    let waiting = super::recovery::next_pending(home)
        || home.join(config::SHARES_PENDING_REL_PATH).is_file();
    let pinned = Pin::load(home)?.is_some();
    if !pinned {
        return Ok(if waiting { RecoveryNext::Create } else { RecoveryNext::Quiet });
    }
    if !waiting {
        return Ok(RecoveryNext::Quiet);
    }
    if super::recovery::seal_exists(home) {
        Ok(RecoveryNext::ShowAgain)
    } else {
        Ok(RecoveryNext::CannotRebuild)
    }
}

fn offer_on_terminal(home: &Path) -> Result<()> {
    match recovery_next(home)? {
        RecoveryNext::Quiet => Ok(()),
        RecoveryNext::Create => identity_init(),
        RecoveryNext::ShowAgain => show_recovery(),
        RecoveryNext::CannotRebuild => {
            println!(
                "This account was created before the phrase was saved here. The password cannot rebuild the words."
            );
            println!("Use the paper copy, or 3 shares with --shares-print.");
            super::recovery::clear_next(home)?;
            Ok(())
        }
    }
}

/// Print the recovery steps and, when there is no account yet, create the phrase.
fn explain_recovery(home: &std::path::Path) -> Result<()> {
    offer_on_terminal(home)
}

/// Before the terminal UI covers the screen, show the phrase and the shares.
/// `Ok(true)` opens Help when the words cannot be rebuilt.
pub fn tui_recovery_offer(home: &std::path::Path) -> Result<bool> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(false);
    }
    match recovery_next(home)? {
        RecoveryNext::Quiet => Ok(false),
        RecoveryNext::Create => {
            identity_init()?;
            prompt("Press Enter to open the terminal UI. The words above were confirmed. ")?;
            Ok(false)
        }
        RecoveryNext::ShowAgain => {
            println!("The recovery phrase and the shares still need to be written down.");
            show_recovery()?;
            prompt("Press Enter to open the terminal UI. ")?;
            Ok(false)
        }
        RecoveryNext::CannotRebuild => {
            println!(
                "This account was created before the phrase was saved here. The password cannot rebuild the words."
            );
            println!("Use the paper copy, or 3 shares with --shares-print.");
            super::recovery::clear_next(home)?;
            Ok(true)
        }
    }
}

/// Show the 24 words again, then a fresh split. Always asks for the password,
/// even when `ACP_PASSWORD` is set. `--show-phrase` is the same screen.
pub fn show_phrase() -> Result<()> {
    show_recovery()
}

/// Show the phrase and then each share. The password is typed, not read from the environment.
pub fn show_recovery() -> Result<()> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!(
            "--show-recovery needs a terminal, so the recovery phrase and the shares are not written to a log or a pipe."
        );
    }
    let home = home()?;
    let pin = Pin::load(&home)?.ok_or_else(|| {
        anyhow::anyhow!("no acp-id is pinned yet. Run --identity-init on a terminal.")
    })?;
    if !super::recovery::seal_exists(&home) {
        bail!(
            "the recovery phrase was not saved on this computer, and the password cannot rebuild it. \
             Use the paper copy, or run --shares-print with at least 3 shares and the password. \
             A phrase created before this file existed was shown once and not stored."
        );
    }
    eprintln!(
        "The words will be printed on this screen and may stay in the terminal's scrollback."
    );
    eprintln!("A screenshot, a photo, or a shared screen is not a safe copy.");
    eprintln!();
    let password = Zeroizing::new(rpassword::prompt_password("Password: ")?);
    let home_open = home.clone();
    let acp_id = pin.acp_id.clone();
    let params = pin.argon2;
    let opened = super::progress::run_busy("Opening the recovery phrase", move || {
        let entropy = super::recovery::open(&home_open, password.as_str())?;
        let phrase = Zeroizing::new(master::phrase_from_entropy(&entropy)?);
        let master = master::derive_master(phrase.as_str(), password.as_str(), params)?;
        master.require_id(&acp_id)?;
        Ok((entropy, phrase, acp_id))
    })?;
    let (entropy, phrase, acp_id) = opened;
    clear_screen();
    println!("Write these 24 words on paper. They are not logged and they are not stored as text.");
    println!();
    println!("{}", phrase.as_str());
    println!();
    let typed = prompt("Type the 24 words to confirm you wrote them down, then Enter: ")?;
    if master::normalize_phrase(&typed) != master::normalize_phrase(phrase.as_str()) {
        println!("Those words do not match. The phrase is still on screen. The shares were not shown.");
        return Ok(());
    }
    println!("Those words match the saved phrase.");
    if let Err(err) = present_shares(acp_id.as_str(), &entropy, true) {
        mark_shares_pending(&home)?;
        return Err(err);
    }
    clear_shares_pending(&home)?;
    note_shares_checked(&home)?;
    super::recovery::clear_next(&home)?;
    let shown = super::recovery::seal_path(&home).display().to_string();
    println!(
        "The terminal was cleared. The words remain encrypted at {shown}. Run --show-recovery to see them again."
    );
    println!(
        "Clearing the screen does not erase a screenshot, and some terminals keep the scrollback anyway."
    );
    Ok(())
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
