use crate::types::{get_mcp_socket_path, text_content};
use anyhow::Result;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

pub fn new_tab(session: &str, args: Value) -> Result<Value> {
    send_to_session_server(session, "new_tab", args)
}

pub fn close_tab(session: &str, args: Value) -> Result<Value> {
    send_to_session_server(session, "close_tab", args)
}



pub fn query_tab_names(session: &str) -> Result<Value> {
    send_to_session_server(session, "query_tab_names", serde_json::json!({}))
}

/// Helper to send requests to session's MCP server
fn send_to_session_server(session: &str, operation: &str, args: Value) -> Result<Value> {
    let socket_path = get_mcp_socket_path(session)?;

    if !socket_path.exists() {
        anyhow::bail!("MCP server socket not found for session '{}'", session);
    }

    let mut stream = UnixStream::connect(&socket_path)?;

    // Send request
    let request = serde_json::json!({
        "operation": operation,
        "args": args
    });

    writeln!(stream, "{}", serde_json::to_string(&request)?)?;
    stream.flush()?;

    // Read response
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    let result: Value = serde_json::from_str(&response)?;

    if let Some(error) = result.get("error") {
        anyhow::bail!("Server error: {}", error);
    }

    // Format response for MCP
    if let Some(tabs) = result.get("tabs") {
        // Special handling for query_tab_names
        Ok(text_content(serde_json::to_string_pretty(&tabs)?))
    } else if let Some(message) = result.get("message") {
        Ok(text_content(message.as_str().unwrap_or("")))
    } else {
        Ok(text_content(serde_json::to_string_pretty(&result)?))
    }
}
