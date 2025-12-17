use crate::types::get_mcp_socket_path;
use anyhow::Result;

/// Check if a session has MCP enabled (socket exists)
pub fn check_mcp_enabled(session_name: &str) -> bool {
    if let Ok(socket_path) = get_mcp_socket_path(session_name) {
        socket_path.exists()
    } else {
        false
    }
}

/// Session info with MCP status
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub name: String,
    pub mcp_enabled: bool,
    pub is_active: bool,
}

/// Get all sessions with MCP status
pub fn list_sessions_with_mcp_status() -> Result<Vec<SessionInfo>> {
    use zellij_utils::sessions::get_sessions;

    let sessions = get_sessions().map_err(|e| anyhow::anyhow!("{:?}", e))?;
    let current_session = std::env::var("ZELLIJ_SESSION_NAME").ok();

    let mut session_infos = Vec::new();
    for (name, _created) in sessions {
        let is_active = current_session.as_ref() == Some(&name);
        let mcp_enabled = check_mcp_enabled(&name);

        session_infos.push(SessionInfo {
            name,
            mcp_enabled,
            is_active,
        });
    }

    Ok(session_infos)
}
