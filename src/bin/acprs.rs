//! Short command for the `acp-red-star` binary.
//!
//! `cargo install` cannot create a symlink, so this is a real binary that
//! runs the same program. `--version` stays `acp-red-star`. The usage line
//! uses `acprs`, the same way `acp-rs-scraper --help` uses that name. There
//! is no deprecation warning.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    acp_red_star::app::run().await
}
