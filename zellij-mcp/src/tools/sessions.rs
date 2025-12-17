use crate::types::text_content;
use anyhow::{Context, Result};
use serde_json::Value;

/// Returns a command string to attach to a session.
/// Note: This function returns a command to run rather than attaching directly,
/// because attaching to a session requires spawning a new terminal process
/// which is outside the scope of MCP operations.
pub fn attach_session(args: Value) -> Result<Value> {
    let session_name = args
        .get("session_name")
        .and_then(|s| s.as_str())
        .context("Missing session_name")?;

    Ok(text_content(format!("Run: zellij attach {}", session_name)))
}

/// Returns a command string to create a new session.
/// Note: This function returns a command to run rather than creating the session directly,
/// because creating a new session requires spawning a new Zellij process and terminal
/// which is outside the scope of MCP operations. Users should run the returned command
/// in their terminal to create the session.
pub fn new_session(args: Value) -> Result<Value> {
    let session_name = args
        .get("session_name")
        .and_then(|s| s.as_str())
        .context("Missing session_name")?;

    let layout = args.get("layout").and_then(|l| l.as_str());

    let cmd = if let Some(layout) = layout {
        format!("zellij --session {} --layout {}", session_name, layout)
    } else {
        format!("zellij --session {}", session_name)
    };

    Ok(text_content(format!("Run: {}", cmd)))
}

pub fn kill_session(args: Value) -> Result<Value> {
    let session_name = args
        .get("session_name")
        .and_then(|s| s.as_str())
        .context("Missing session_name")?;

    Ok(text_content(format!(
        "Would kill session: {}",
        session_name
    )))
}

pub fn get_session_info(session: &str) -> Result<Value> {
    use crate::types::get_mcp_socket_path;
    use zellij_utils::sessions::get_sessions;

    // Use Zellij's internal session utilities instead of running external command
    let sessions =
        get_sessions().map_err(|e| anyhow::anyhow!("Failed to get sessions: {:?}", e))?;

    // Find the requested session
    for (name, created) in sessions {
        if name == session {
            let mcp_socket = get_mcp_socket_path(session)?;
            let mcp_enabled = mcp_socket.exists();

            // Format the duration in a human-readable way
            let created_str = format!("{} seconds ago", created.as_secs());

            let info = format!(
                "Created: {}\nMCP enabled: {}\nSocket: {}",
                created_str,
                mcp_enabled,
                mcp_socket.display()
            );
            return Ok(text_content(format!("Session: {}\n{}", session, info)));
        }
    }

    Ok(text_content(format!("Session '{}' not found", session)))
}
