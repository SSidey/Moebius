use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;

use crate::mcp::http_server::run_http_server;
use crate::mcp::server::McpServer;
use crate::mcp::session::McpSession;
use crate::tools::RealToolExecutor;

pub enum Transport {
    Stdio,
    Http,
}

pub fn serve(
    working_dir: PathBuf,
    transport: Transport,
    port: u16,
    host: String,
    public_url: Option<String>,
) -> anyhow::Result<()> {
    match transport {
        Transport::Stdio => {
            let session = McpSession::new(working_dir);
            let state = Arc::clone(&session.state);
            let executor = RealToolExecutor::new_mcp(state);
            let mut server = McpServer::new(session, executor);
            server.run().context("MCP stdio server error")
        }
        Transport::Http => {
            let url = public_url
                .ok_or_else(|| anyhow::anyhow!(
                    "--public-url is required when using --transport http. \
                     Example: moeb serve --transport http --public-url https://my-tunnel.example.com"
                ))?;
            tokio::runtime::Runtime::new()
                .context("failed to create tokio runtime")?
                .block_on(run_http_server(working_dir, url, host, port))
                .context("MCP HTTP server error")
        }
    }
}
