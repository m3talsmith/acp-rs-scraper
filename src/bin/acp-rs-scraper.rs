//! Deprecated name for the `acp-red-star` binary.
//!
//! Prints one line on stderr and runs the same program, so scripts that still
//! call `acp-rs-scraper` keep working through version 3.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    eprintln!("acp-rs-scraper is now acp-red-star. This name keeps working through version 3.");
    acp_red_star::app::run().await
}
