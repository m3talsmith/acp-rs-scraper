//! `acp-rs-scraper` binary: parses the command line and runs [`acp_rs_scraper::app::run`].

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    acp_rs_scraper::app::run().await
}
