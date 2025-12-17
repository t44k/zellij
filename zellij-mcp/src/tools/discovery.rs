use crate::session::list_sessions_with_mcp_status;
use crate::types::{get_mcp_socket_path, text_content};
use anyhow::Result;
use serde_json::Value;

/// List all sessions with MCP status
pub fn list_sessions() -> Result<Value> {
    let sessions = list_sessions_with_mcp_status()?;

    if sessions.is_empty() {
        return Ok(text_content("No active sessions found"));
    }

    let mut lines = vec!["Available sessions:".to_string()];
    for session_info in sessions {
        let status = if session_info.mcp_enabled {
            "mcp: enabled"
        } else {
            "mcp: disabled"
        };
        let active = if session_info.is_active {
            ", active"
        } else {
            ""
        };
        lines.push(format!("- {} ({status}{active})", session_info.name));
    }

    Ok(text_content(lines.join("\n")))
}

/// Get current session from environment
pub fn get_current_session() -> Result<Value> {
    match std::env::var("ZELLIJ_SESSION_NAME") {
        Ok(session) => Ok(text_content(format!("Current session: {}", session))),
        Err(_) => Ok(text_content("Not inside a Zellij session")),
    }
}

/// Health check
pub fn health_check(args: Value) -> Result<Value> {
    let session = if let Some(s) = args.get("session").and_then(|s| s.as_str()) {
        Some(s.to_string())
    } else {
        std::env::var("ZELLIJ_SESSION_NAME").ok()
    };

    let mut status_lines = vec!["MCP Status:".to_string(), "- Version: 0.44.0".to_string()];

    if let Some(session_name) = session {
        status_lines.push(format!("- Session: {}", session_name));

        if let Ok(socket_path) = get_mcp_socket_path(&session_name) {
            let connected = socket_path.exists();
            status_lines.push(format!(
                "- Socket: {}",
                if connected { "connected" } else { "not found" }
            ));
            status_lines.push(format!("- Path: {}", socket_path.display()));
        }
    } else {
        status_lines.push("- Session: not specified".to_string());
    }

    status_lines.push("- Config: enabled=true, max_lines=10000, timeout=5s".to_string());

    Ok(text_content(status_lines.join("\n")))
}
