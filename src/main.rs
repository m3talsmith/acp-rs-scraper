//! `acp-red-star` binary: parses the command line and runs [`acp_red_star::app::run`].

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    acp_red_star::app::run().await
}
