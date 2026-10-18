//! `--identity-init` and `--acp-id`. The phrase is printed once, to the terminal,
//! and is never logged.

use anyhow::{Result, bail};
use zeroize::Zeroizing;

use super::master::{self, Master};
use super::password::{check_password, default_params};
use super::pin::Pin;
use crate::cli::prompt;
use crate::config::home;

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
    let entropy = master::new_entropy();
    let phrase = master::phrase_from_entropy(&entropy)?;
    let master = master::derive_master(&phrase, password.as_str(), default_params())?;
    println!();
    println!("Write these 24 words down. They are not stored, not logged, and not copied for you.");
    println!("There is no password reset. The password is not part of the phrase.");
    println!();
    println!("    {phrase}");
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
