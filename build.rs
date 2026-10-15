//! Ensures git uses the repo's tracked hooks (.githooks), so the
//! credential-blocking pre-commit hook is active for every clone.
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.githooks/pre-commit");
    println!("cargo:rerun-if-changed=.git/config");

    // Not a git checkout (e.g. built from a crates.io tarball): nothing to do.
    if !Path::new(".git").exists() || !Path::new(".githooks").is_dir() {
        return;
    }

    let current = Command::new("git")
        .args(["config", "--local", "--get", "core.hooksPath"])
        .output();

    match current {
        Ok(out) if String::from_utf8_lossy(&out.stdout).trim() == ".githooks" => {}
        Ok(_) => {
            let set = Command::new("git")
                .args(["config", "--local", "core.hooksPath", ".githooks"])
                .status();
            match set {
                Ok(s) if s.success() => {
                    println!("cargo:warning=Configured git core.hooksPath=.githooks (credential pre-commit hook enabled)");
                }
                _ => println!(
                    "cargo:warning=Could not set git core.hooksPath; run: git config core.hooksPath .githooks"
                ),
            }
        }
        Err(_) => println!(
            "cargo:warning=git not found; run `git config core.hooksPath .githooks` to enable the credential pre-commit hook"
        ),
    }
}
