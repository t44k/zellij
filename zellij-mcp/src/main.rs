use anyhow::Result;
use zellij_mcp::client::run_mcp_client;

fn main() -> Result<()> {
    env_logger::init();
    run_mcp_client()
}