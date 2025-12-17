use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Get the MCP socket path for a session
pub fn get_mcp_socket_path(session_name: &str) -> Result<PathBuf> {
    let mut sock_dir = dirs::runtime_dir()
        .or_else(|| dirs::cache_dir())
        .context("Cannot find runtime directory")?;
    sock_dir.push("zellij");
    sock_dir.push(format!("{}.mcp.sock", session_name));
    Ok(sock_dir)
}

/// Parse pane ID from string format ("terminal_1" or "plugin_2")
pub fn parse_pane_id(args: &Value) -> Result<PaneIdParsed> {
    let pane_id_str = args
        .get("pane_id")
        .and_then(|p| p.as_str())
        .context("Missing pane_id")?;

    if let Some(id_str) = pane_id_str.strip_prefix("terminal_") {
        let id = id_str.parse::<u32>().context("Invalid terminal pane ID")?;
        Ok(PaneIdParsed::Terminal(id))
    } else if let Some(id_str) = pane_id_str.strip_prefix("plugin_") {
        let id = id_str.parse::<u32>().context("Invalid plugin pane ID")?;
        Ok(PaneIdParsed::Plugin(id))
    } else {
        anyhow::bail!("Invalid pane ID format: {}", pane_id_str)
    }
}

/// Parsed pane ID
#[derive(Debug, Clone, Copy)]
pub enum PaneIdParsed {
    Terminal(u32),
    Plugin(u32),
}

impl PaneIdParsed {
    pub fn to_string(&self) -> String {
        match self {
            PaneIdParsed::Terminal(id) => format!("terminal_{}", id),
            PaneIdParsed::Plugin(id) => format!("plugin_{}", id),
        }
    }
}

/// Helper to create tool definition JSON
pub fn tool_def(name: &str, description: &str, properties: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties
        }
    })
}

/// Extract session parameter from arguments with smart default
pub fn get_session_from_args(args: &Value) -> Result<String> {
    // 1. Check if session explicitly provided
    if let Some(session) = args.get("session").and_then(|s| s.as_str()) {
        return Ok(session.to_string());
    }

    // 2. Fall back to environment variable
    if let Ok(session) = std::env::var("ZELLIJ_SESSION_NAME") {
        return Ok(session);
    }

    // 3. Try to get first available session from the session list
    use crate::session::list_sessions_with_mcp_status;
    match list_sessions_with_mcp_status() {
        Ok(sessions) => {
            if let Some(first_session) = sessions.first() {
                return Ok(first_session.name.clone());
            }
            anyhow::bail!("No active Zellij sessions found. Please start a session with: zellij")
        },
        Err(e) => anyhow::bail!("Failed to get sessions: {}", e),
    }
}

/// Format MCP content response
pub fn text_content(text: impl Into<String>) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": text.into()
        }]
    })
}
