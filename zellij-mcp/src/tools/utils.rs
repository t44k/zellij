use crate::types::text_content;
use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

pub fn rename_session(session: &str, args: Value) -> Result<Value> {
    use crate::types::get_mcp_socket_path;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let socket_path = get_mcp_socket_path(session)?;

    if !socket_path.exists() {
        anyhow::bail!("MCP server socket not found for session '{}'", session);
    }

    let mut stream = UnixStream::connect(&socket_path)?;

    // Send request
    let request = serde_json::json!({
        "operation": "rename_session",
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
    if let Some(message) = result.get("message") {
        Ok(text_content(message.as_str().unwrap_or("")))
    } else {
        Ok(text_content(serde_json::to_string_pretty(&result)?))
    }
}

pub fn dump_screen(session: &str, args: Value) -> Result<Value> {
    // Get optional path parameter
    let path = args.get("path").and_then(|p| p.as_str());
    let full = args.get("full").and_then(|f| f.as_bool()).unwrap_or(false);

    // Create temp file if no path specified
    let temp_path;
    let output_path = if let Some(p) = path {
        p
    } else {
        temp_path = format!("/tmp/zellij-dump-{}.txt", std::process::id());
        &temp_path
    };

    // Build command
    let mut cmd_args = vec!["--session", session, "action", "dump-screen", output_path];
    if full {
        cmd_args.insert(4, "--full-screen");
    }

    // Execute dump-screen
    let output = Command::new("zellij")
        .args(&cmd_args)
        .output()
        .context("Failed to execute zellij dump-screen")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Zellij dump-screen failed: {}", stderr);
    }

    // Read the dumped content
    let content = std::fs::read_to_string(output_path)
        .context(format!("Failed to read dumped screen from {}", output_path))?;

    // Clean up temp file if we created one
    if path.is_none() {
        let _ = std::fs::remove_file(output_path);
    }

    Ok(text_content(format!("Screen content:\n{}", content)))
}
