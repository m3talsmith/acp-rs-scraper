//! The deprecated binary name forwards to ACP Red Star.

use std::process::Command;

#[test]
fn the_old_binary_name_warns_and_forwards_version() {
    let exe = env!("CARGO_BIN_EXE_acp-rs-scraper");
    let out = Command::new(exe).arg("--version").output().expect("run acp-rs-scraper");
    let err = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr:\n{err}\nstdout:\n{stdout}");
    assert!(err.contains("acp-rs-scraper is now acp-red-star"), "{err}");
    assert!(err.contains("through version 3"), "{err}");
    assert!(stdout.starts_with("acp-red-star "), "{stdout}");
}

#[test]
fn acprs_builds_and_prints_the_primary_version() {
    let exe = env!("CARGO_BIN_EXE_acprs");
    let out = Command::new(exe).arg("--version").output().expect("run acprs");
    let err = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr:\n{err}\nstdout:\n{stdout}");
    assert!(err.is_empty(), "{err}");
    assert!(stdout.starts_with("acp-red-star "), "{stdout}");
    let help = Command::new(exe).arg("--help").output().expect("acprs --help");
    let help_out = String::from_utf8_lossy(&help.stdout);
    let help_err = String::from_utf8_lossy(&help.stderr);
    assert!(help.status.success(), "stderr:\n{help_err}\nstdout:\n{help_out}");
    assert!(help_out.contains("Usage: acprs"), "{help_out}");
    assert!(!help_err.contains("is now acp-red-star"), "{help_err}");
}
