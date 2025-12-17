use anyhow::{Context, Result};
use log;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

pub fn get_mcp_socket_path(session_name: &str) -> Result<PathBuf> {
    let mut sock_dir = dirs::runtime_dir()
        .or_else(|| dirs::cache_dir())
        .context("Cannot find runtime directory")?;
    sock_dir.push("zellij");
    std::fs::create_dir_all(&sock_dir)?;
    sock_dir.push(format!("{}.mcp.sock", session_name));
    Ok(sock_dir)
}

pub fn mcp_server_main(socket_path: PathBuf) -> Result<()> {
    log::info!("[MCP] Starting placeholder MCP server at {:?}", socket_path);

    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path).context("Failed to bind MCP socket")?;

    log::info!("[MCP] MCP server listening on {:?}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                log::info!("[MCP] Client connected");
                if let Err(e) = handle_client(stream) {
                    log::error!("[MCP] Client error: {}", e);
                }
                log::info!("[MCP] Client disconnected");
            },
            Err(e) => {
                log::error!("[MCP] Connection error: {}", e);
            },
        }
    }

    Ok(())
}

fn handle_client(stream: UnixStream) -> Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = serde_json::from_str(&line).context("Invalid JSON")?;
        let response = handle_operation(request);

        writeln!(writer, "{}", serde_json::to_string(&response)?)?;
        writer.flush()?;
    }

    Ok(())
}

fn handle_operation(request: Value) -> Value {
    let operation = match request.get("operation").and_then(|o| o.as_str()) {
        Some(op) => op,
        None => return json!({"error": "Missing operation"}),
    };

    log::debug!("[MCP] Operation: {}", operation);

    json!({"content": "This is a placeholder server. The actual MCP server runs in zellij-server."})
}
