use crate::types::{get_mcp_socket_path, text_content};
use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

/// List all panes in session
pub fn list_panes(session: &str) -> Result<Value> {
    send_to_session_server(session, "list_panes", serde_json::json!({}))
}

/// Read pane content
pub fn read_pane(session: &str, args: Value) -> Result<Value> {
    // Validate tab_index is provided to avoid ambiguity
    if !args.get("tab_index").is_some() {
        anyhow::bail!("tab_index is required for read_pane to avoid ambiguity between tabs");
    }
    send_to_session_server(session, "read_pane", args)
}

/// Send raw key events to a pane without bracketed-paste wrapping
pub fn send_keys(session: &str, args: Value) -> Result<Value> {
    if !args.get("tab_index").is_some() {
        anyhow::bail!("tab_index is required for send_keys to avoid ambiguity between tabs");
    }
    if !args.get("pane_id").is_some() {
        anyhow::bail!("pane_id is required for send_keys");
    }
    if !args.get("keys").is_some() {
        anyhow::bail!("keys array is required for send_keys");
    }
    send_to_session_server(session, "send_keys", args)
}

/// Write to pane (unfocused)
pub fn write_to_pane(session: &str, args: Value) -> Result<Value> {
    // Validate tab_index is provided to avoid ambiguity
    if !args.get("tab_index").is_some() {
        anyhow::bail!("tab_index is required for write_to_pane to avoid ambiguity between tabs");
    }
    send_to_session_server(session, "write_to_pane", args)
}

/// Run command in pane (unfocused)
pub fn run_command_in_pane(session: &str, args: Value) -> Result<Value> {
    // Validate tab_index is provided to avoid ambiguity
    if !args.get("tab_index").is_some() {
        anyhow::bail!("tab_index is required for run_command_in_pane to avoid ambiguity between tabs");
    }
    
    let pane_id = args.get("pane_id").context("Missing pane_id")?;
    let command = args
        .get("command")
        .and_then(|c| c.as_str())
        .context("Missing command")?;

    let write_args = serde_json::json!({
        "pane_id": pane_id,
        "text": command,
        "submit": true
    });

    send_to_session_server(session, "write_to_pane", write_args)
}

/// Focus pane
pub fn focus_pane(session: &str, args: Value) -> Result<Value> {
    // Validate tab_index is provided to avoid ambiguity
    if !args.get("tab_index").is_some() {
        anyhow::bail!("tab_index is required for focus_pane to avoid ambiguity between tabs");
    }
    send_to_session_server(session, "focus_pane", args)
}

/// New pane
pub fn new_pane(session: &str, args: Value) -> Result<Value> {
    // Validate tab_index is provided to avoid ambiguity
    if !args.get("tab_index").is_some() {
        anyhow::bail!("tab_index is required for new_pane to avoid ambiguity between tabs");
    }
    send_to_session_server(session, "new_pane", args)
}

/// Close pane
pub fn close_pane(session: &str, args: Value) -> Result<Value> {
    // Validate tab_index is provided to avoid ambiguity
    if !args.get("tab_index").is_some() {
        anyhow::bail!("tab_index is required for close_pane to avoid ambiguity between tabs");
    }
    send_to_session_server(session, "close_pane", args)
}

/// Helper to send requests to session's MCP server
/// Falls back to CLI commands if socket doesn't exist
fn send_to_session_server(session: &str, operation: &str, args: Value) -> Result<Value> {
    let socket_path = get_mcp_socket_path(session)?;

    // Try socket first (preferred method when MCP server is running)
    if socket_path.exists() {
        match try_socket_communication(session, &socket_path, operation, &args) {
            Ok(result) => return Ok(result),
            Err(e) => {
                log::warn!(
                    "[MCP] Socket communication failed, falling back to CLI: {}",
                    e
                );
            },
        }
    }

    // Fall back to CLI commands
    execute_via_cli(session, operation, args)
}

/// Try to communicate via Unix socket
fn try_socket_communication(
    session: &str,
    socket_path: &std::path::Path,
    operation: &str,
    args: &Value,
) -> Result<Value> {
    let mut stream = UnixStream::connect(socket_path)
        .context(format!("Failed to connect to session '{}'", session))?;

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

    // Return as MCP content format
    if let Some(content) = result.get("content") {
        Ok(serde_json::json!({
            "content": [{
                "type": "text",
                "text": content.as_str().unwrap_or("")
            }]
        }))
    } else if result
        .get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(false)
    {
        // For successful operations with structured data (like list_panes)
        // Format the result nicely
        let text = if let Some(panes) = result.get("panes") {
            serde_json::to_string_pretty(&panes).unwrap_or_else(|_| format!("{:?}", panes))
        } else if let Some(message) = result.get("message") {
            message.to_string()
        } else {
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result))
        };

        Ok(serde_json::json!({
            "content": [{
                "type": "text",
                "text": text
            }]
        }))
    } else {
        Ok(text_content(format!("Operation '{}' completed", operation)))
    }
}

/// Execute operation via CLI commands (fallback method)
fn execute_via_cli(session: &str, operation: &str, args: Value) -> Result<Value> {
    use std::process::Command;

    match operation {
        "write_to_pane" => {
            let text = args
                .get("text")
                .and_then(|t| t.as_str())
                .context("Missing text parameter")?;
            let submit = args
                .get("submit")
                .and_then(|s| s.as_bool())
                .unwrap_or(false);

            // Execute via zellij action write-chars
            let output = Command::new("zellij")
                .args(&["--session", session, "action", "write-chars", text])
                .output()
                .context("Failed to execute zellij command")?;

            if submit {
                // Send newline as byte 10 (LF)
                Command::new("zellij")
                    .args(&["--session", session, "action", "write", "10"])
                    .output()
                    .context("Failed to execute zellij submit command")?;
            }

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Zellij command failed: {}", stderr);
            }

            Ok(text_content(format!(
                "Wrote '{}' to pane{}",
                text,
                if submit { " (submitted)" } else { "" }
            )))
        },
        "new_pane" => {
            let direction = args.get("direction").and_then(|d| d.as_str());
            let command = args.get("command").and_then(|c| c.as_str());
            let cwd = args.get("cwd").and_then(|c| c.as_str());

            let mut zellij_args = vec!["--session", session, "action", "new-pane"];

            if let Some(dir) = direction {
                zellij_args.push("--direction");
                zellij_args.push(dir);
            }

            if let Some(cwd_path) = cwd {
                zellij_args.push("--cwd");
                zellij_args.push(cwd_path);
            }

            if let Some(cmd) = command {
                zellij_args.push("--");
                zellij_args.push(cmd);
            }

            let output = Command::new("zellij")
                .args(&zellij_args)
                .output()
                .context("Failed to execute zellij command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Zellij command failed: {}", stderr);
            }

            Ok(text_content(format!(
                "Created new pane{}",
                if let Some(cmd) = command {
                    format!(" running: {}", cmd)
                } else {
                    String::new()
                }
            )))
        },
        "focus_pane" => {
            // For now, focus operations are limited without direct server access
            Ok(text_content(
                "Focus pane via CLI not fully supported yet. Please use Zellij keybindings.",
            ))
        },
        "close_pane" => {
            let output = Command::new("zellij")
                .args(&["--session", session, "action", "close-pane"])
                .output()
                .context("Failed to execute zellij command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Zellij command failed: {}", stderr);
            }

            Ok(text_content("Closed pane"))
        },
        "list_panes" => {
            // Use dump-layout to get pane information
            let output = Command::new("zellij")
                .args(&["--session", session, "action", "dump-layout"])
                .output()
                .context("Failed to execute zellij command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Zellij command failed: {}", stderr);
            }

            let layout_output = String::from_utf8_lossy(&output.stdout);
            Ok(text_content(format!("Session layout:\n{}", layout_output)))
        },
        "read_pane" => {
            // NOTE: This CLI fallback is only used when the MCP socket server is unavailable.
            // The socket-based server (in zellij-server/src/mcp_server.rs) uses direct IPC
            // without temporary files. This fallback must use temp files because the CLI
            // `dump-screen` action is the only way to get pane content via external commands.
            
            // Extract parameters
            let pane_number = args
                .get("pane_id")
                .and_then(|p| p.as_str())
                .and_then(|s| s.parse::<usize>().ok());
            let lines = args.get("lines").and_then(|l| l.as_u64());
            let offset = args.get("offset").and_then(|o| o.as_u64()).unwrap_or(0);

            let temp_file = format!("/tmp/zellij-pane-read-{}.txt", std::process::id());

            // If pane_id is specified, we need to navigate to it
            if let Some(target_pane) = pane_number {
                // Strategy: Focus the target pane, read it, then restore focus
                // Step 1: Get current layout to count panes and find focused one
                let layout_output = Command::new("zellij")
                    .args(&["--session", session, "action", "dump-layout"])
                    .output()
                    .context("Failed to get layout")?;

                let layout = String::from_utf8_lossy(&layout_output.stdout);

                // Count how many panes exist by counting "pane" keywords (rough estimate)
                let pane_count = layout.matches("pane").count();

                // Navigate to target pane using focus-next-pane cycling
                // We'll cycle through all panes to reach our target
                for _ in 0..target_pane {
                    Command::new("zellij")
                        .args(&["--session", session, "action", "focus-next-pane"])
                        .output()
                        .context("Failed to navigate panes")?;

                    // Small delay to ensure focus change is processed
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                // Read the now-focused target pane
                Command::new("zellij")
                    .args(&["--session", session, "action", "dump-screen", &temp_file])
                    .output()
                    .context("Failed to dump screen")?;

                // Restore original focus by cycling back
                let restore_cycles = if pane_count > target_pane {
                    pane_count - target_pane
                } else {
                    0
                };

                for _ in 0..restore_cycles {
                    Command::new("zellij")
                        .args(&["--session", session, "action", "focus-next-pane"])
                        .output()
                        .context("Failed to restore focus")?;

                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            } else {
                // No pane_id specified, read currently focused pane
                let output = Command::new("zellij")
                    .args(&["--session", session, "action", "dump-screen", &temp_file])
                    .output()
                    .context("Failed to dump screen")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Zellij dump-screen failed: {}", stderr);
                }
            }

            // Read the dumped content
            let content =
                std::fs::read_to_string(&temp_file).context("Failed to read dumped screen")?;

            // Clean up temp file
            let _ = std::fs::remove_file(&temp_file);

            // Process lines with offset and limit
            let all_lines: Vec<&str> = content.lines().collect();
            let total_lines = all_lines.len();

            let start = offset as usize;
            let end = if let Some(max_lines) = lines {
                std::cmp::min(start + max_lines as usize, total_lines)
            } else {
                total_lines
            };

            if start >= total_lines {
                return Ok(text_content(format!(
                    "Offset {} is beyond total lines {}. No content to show.",
                    offset, total_lines
                )));
            }

            let selected_lines = &all_lines[start..end];
            let result = selected_lines.join("\n");

            Ok(text_content(format!(
                "Pane {} content (lines {}-{} of {}):\n{}",
                pane_number
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "focused".to_string()),
                start,
                end.saturating_sub(1),
                total_lines,
                result
            )))
        },
        _ => {
            anyhow::bail!("Unsupported operation via CLI: {}", operation)
        },
    }
}
