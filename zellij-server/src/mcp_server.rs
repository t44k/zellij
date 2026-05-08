//! MCP (Model Context Protocol) server for Zellij
//! This runs as a background thread and provides access to pane content via Unix socket

use anyhow::{Context, Result};
use log;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use zellij_utils::channels::SenderWithContext;
use zellij_utils::data::PaneScrollbackResponse;

use crate::panes::PaneId;
use crate::pty::PtyInstruction;
use crate::screen::ScreenInstruction;

pub fn start_mcp_server(
    socket_path: PathBuf,
    screen_sender: SenderWithContext<ScreenInstruction>,
    pty_sender: SenderWithContext<PtyInstruction>,
    session_name: String,
) -> Result<thread::JoinHandle<()>> {
    let handle = thread::Builder::new()
        .name("mcp_server".to_string())
        .spawn(move || {
            if let Err(e) = run_mcp_server(socket_path, screen_sender, pty_sender, session_name) {
                log::error!("[MCP] Server error: {}", e);
            }
        })?;

    Ok(handle)
}

fn run_mcp_server(
    socket_path: PathBuf,
    screen_sender: SenderWithContext<ScreenInstruction>,
    pty_sender: SenderWithContext<PtyInstruction>,
    session_name: String,
) -> Result<()> {
    log::info!(
        "[MCP] Starting MCP server for session '{}' at {:?}",
        session_name,
        socket_path
    );

    let _ = std::fs::remove_file(&socket_path);

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&socket_path).context("Failed to bind MCP socket")?;

    log::info!("[MCP] MCP server listening on {:?}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                log::info!("[MCP] Client connected");
                if let Err(e) = handle_client(stream, &screen_sender, &pty_sender) {
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

fn handle_client(
    stream: UnixStream,
    screen_sender: &SenderWithContext<ScreenInstruction>,
    pty_sender: &SenderWithContext<PtyInstruction>,
) -> Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = serde_json::from_str(&line).context("Invalid JSON")?;
        let response = handle_operation(request, screen_sender, pty_sender);

        writeln!(writer, "{}", serde_json::to_string(&response)?)?;
        writer.flush()?;
    }

    Ok(())
}

fn handle_operation(
    request: Value,
    screen_sender: &SenderWithContext<ScreenInstruction>,
    pty_sender: &SenderWithContext<PtyInstruction>,
) -> Value {
    let operation = match request.get("operation").and_then(|o| o.as_str()) {
        Some(op) => op,
        None => return json!({"error": "Missing operation"}),
    };

    let args = request.get("args").cloned().unwrap_or(json!({}));

    log::debug!("[MCP] Operation: {}, args: {:?}", operation, args);

    match operation {
        "read_pane" => handle_read_pane(args, screen_sender),
        "list_panes" => handle_list_panes(args, screen_sender),
        "write_to_pane" => handle_write_to_pane(args, screen_sender),
        "send_keys" => handle_send_keys(args, screen_sender),
        "focus_pane" => handle_focus_pane(args, screen_sender),
        "new_pane" => handle_new_pane(args, pty_sender),
        "close_pane" => handle_close_pane(args, screen_sender),
        "new_tab" => handle_new_tab(args, screen_sender),
        "close_tab" => handle_close_tab(screen_sender),
        
        "query_tab_names" => handle_query_tab_names(screen_sender),
        "rename_session" => handle_rename_session(args, screen_sender),
        "launch_plugin" => handle_launch_plugin(args, screen_sender),
        "dump_layout" => handle_dump_layout(args, screen_sender),
        "list_aliases" => handle_list_aliases(),
        _ => json!({"error": format!("Unknown operation: {}", operation)}),
    }
}

fn handle_read_pane(args: Value, screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    let pane_id_str = match args.get("pane_id").and_then(|p| p.as_str()) {
        Some(id) => id,
        None => return json!({"error": "Missing pane_id"}),
    };

    let pane_id = match parse_pane_id(pane_id_str) {
        Ok(id) => id,
        Err(e) => return json!({"error": e}),
    };

    let include_scrollback = args
        .get("include_scrollback")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    let lines = args.get("lines").and_then(|l| l.as_u64());
    let offset = args.get("offset").and_then(|o| o.as_u64()).unwrap_or(0);

    // Use direct IPC with response channel instead of temporary files
    let (response_tx, response_rx) = crossbeam::channel::bounded(1);

    if let Err(e) = screen_sender.send(ScreenInstruction::GetPaneScrollback {
        pane_id,
        client_id: 0, // MCP server doesn't have a specific client ID
        get_full_scrollback: include_scrollback,
        response_channel: response_tx,
    }) {
        return json!({"error": format!("Failed to send scrollback request: {}", e)});
    }

    // Wait for response with timeout
    let pane_contents = match response_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(PaneScrollbackResponse::Ok(contents)) => contents,
        Ok(PaneScrollbackResponse::Err(err)) => {
            return json!({"error": format!("Failed to get pane contents: {}", err)});
        },
        Err(e) => {
            return json!({"error": format!("Timeout waiting for pane contents: {}", e)});
        },
    };

    // Combine all lines: scrollback + viewport + lines below
    let mut all_lines = Vec::new();
    all_lines.extend(pane_contents.lines_above_viewport);
    all_lines.extend(pane_contents.viewport);
    all_lines.extend(pane_contents.lines_below_viewport);

    let total_lines = all_lines.len();

    let start = offset as usize;
    let end = if let Some(max_lines) = lines {
        std::cmp::min(start + max_lines as usize, total_lines)
    } else {
        total_lines
    };

    if start >= total_lines {
        return json!({
            "content": format!("Offset {} is beyond total lines {}. No content to show.", offset, total_lines)
        });
    }

    let selected_lines = &all_lines[start..end];
    let result = selected_lines.join("\n");

    json!({
        "content": format!("Pane {} content (lines {}-{} of {}):\n{}",
            pane_id_str, start, end.saturating_sub(1), total_lines, result)
    })
}

fn parse_pane_id(pane_id_str: &str) -> Result<PaneId, String> {
    if let Some(id_str) = pane_id_str.strip_prefix("terminal_") {
        let id = id_str
            .parse::<u32>()
            .map_err(|_| format!("Invalid terminal pane ID: {}", pane_id_str))?;
        Ok(PaneId::Terminal(id))
    } else if let Some(id_str) = pane_id_str.strip_prefix("plugin_") {
        let id = id_str
            .parse::<u32>()
            .map_err(|_| format!("Invalid plugin pane ID: {}", pane_id_str))?;
        Ok(PaneId::Plugin(id))
    } else {
        Err(format!("Invalid pane ID format: {}", pane_id_str))
    }
}

fn handle_list_panes(_args: Value, screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    let (response_tx, response_rx) = crossbeam::channel::bounded(1);

    if let Err(e) = screen_sender.send(ScreenInstruction::GetPaneManifest {
        response_channel: response_tx,
    }) {
        return json!({"error": format!("Failed to send GetPaneManifest instruction: {}", e)});
    }

    match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(pane_manifest) => {
            let mut panes_json = serde_json::Map::new();
            for (tab_index, panes) in pane_manifest.panes {
                let panes_list: Vec<serde_json::Value> = panes
                    .iter()
                    .map(|pane| {
                        json!({
                            "id": if pane.is_plugin {
                                format!("plugin_{}", pane.id)
                            } else {
                                format!("terminal_{}", pane.id)
                            },
                            "is_plugin": pane.is_plugin,
                            "is_focused": pane.is_focused,
                            "is_fullscreen": pane.is_fullscreen,
                            "is_floating": pane.is_floating,
                            "title": pane.title.clone(),
                        })
                    })
                    .collect();
                panes_json.insert(format!("tab_{}", tab_index), json!(panes_list));
            }

            json!({
                "success": true,
                "panes": panes_json
            })
        },
        Err(e) => {
            json!({"error": format!("Timeout waiting for pane manifest: {}", e)})
        },
    }
}

fn handle_write_to_pane(
    args: Value,
    screen_sender: &SenderWithContext<ScreenInstruction>,
) -> Value {
    let pane_id_str = match args.get("pane_id").and_then(|p| p.as_str()) {
        Some(id) => id,
        None => return json!({"error": "Missing pane_id"}),
    };

    let text = match args.get("text").and_then(|t| t.as_str()) {
        Some(t) => t,
        None => return json!({"error": "Missing text"}),
    };

    let submit = args
        .get("submit")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    let pane_id = match parse_pane_id(pane_id_str) {
        Ok(id) => id,
        Err(e) => return json!({"error": e}),
    };

    let mut bytes = text.as_bytes().to_vec();
    if submit {
        bytes.push(b'\n');
    }

    if let Err(e) = screen_sender.send(ScreenInstruction::WriteToPaneId(bytes, pane_id)) {
        return json!({"error": format!("Failed to write to pane: {}", e)});
    }

    json!({"success": true, "message": format!("Text written to pane {}", pane_id_str)})
}

fn handle_focus_pane(args: Value, screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    use zellij_utils::data::ClientId;

    let pane_id_str = match args.get("pane_id").and_then(|p| p.as_str()) {
        Some(id) => id,
        None => return json!({"error": "Missing pane_id"}),
    };

    let pane_id = match parse_pane_id(pane_id_str) {
        Ok(id) => id,
        Err(e) => return json!({"error": e}),
    };

    let client_id: ClientId = 1;

    if let Err(e) = screen_sender.send(ScreenInstruction::FocusPaneWithId(
        pane_id, false, false, client_id, None,
    )) {
        return json!({"error": format!("Failed to focus pane: {}", e)});
    }

    json!({"success": true, "message": format!("Focused pane {}", pane_id_str)})
}

fn handle_new_pane(args: Value, pty_sender: &SenderWithContext<PtyInstruction>) -> Value {
    use crate::pty::ClientTabIndexOrPaneId;
    use zellij_utils::data::{ClientId, Direction, NewPanePlacement};
    use zellij_utils::input::command::TerminalAction;

    let direction = args.get("direction").and_then(|d| d.as_str());
    let placement = if let Some(dir_str) = direction {
        let dir = match dir_str {
            "right" => Direction::Right,
            "down" => Direction::Down,
            "left" => Direction::Left,
            "up" => Direction::Up,
            _ => return json!({"error": format!("Invalid direction: {}", dir_str)}),
        };
        NewPanePlacement::Tiled(Some(dir))
    } else {
        NewPanePlacement::NoPreference
    };

    let terminal_action = args.get("command").and_then(|c| c.as_str()).map(|cmd| {
        TerminalAction::RunCommand(zellij_utils::input::command::RunCommand {
            command: PathBuf::from(cmd.split_whitespace().next().unwrap_or(cmd)),
            args: cmd.split_whitespace().skip(1).map(String::from).collect(),
            cwd: args.get("cwd").and_then(|c| c.as_str()).map(PathBuf::from),
            hold_on_close: false,
            hold_on_start: false,
            originating_plugin: None,
            use_terminal_title: false,
        })
    });

    // Use tab_index to target a specific tab (0-based), so panes land on the correct tab
    // even when it isn't the currently focused one. Falls back to client-based routing
    // (active tab) when tab_index is not supplied.
    let client_id: ClientId = 1;
    let client_or_tab = if let Some(tab_index) = args.get("tab_index").and_then(|t| t.as_u64()) {
        ClientTabIndexOrPaneId::TabIndex(tab_index as usize)
    } else {
        ClientTabIndexOrPaneId::ClientId(client_id)
    };

    if let Err(e) = pty_sender.send(PtyInstruction::SpawnTerminal(
        terminal_action,
        None,
        placement,
        false,
        client_or_tab,
        None,
        false,
    )) {
        return json!({"error": format!("Failed to spawn terminal: {}", e)});
    }

    json!({
        "success": true,
        "message": "New pane spawn requested",
        "placement": direction.unwrap_or("default")
    })
}

fn handle_close_pane(args: Value, screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    let pane_id_str = match args.get("pane_id").and_then(|p| p.as_str()) {
        Some(id) => id,
        None => return json!({"error": "Missing pane_id"}),
    };

    let pane_id = match parse_pane_id(pane_id_str) {
        Ok(id) => id,
        Err(e) => return json!({"error": e}),
    };

    if let Err(e) = screen_sender.send(ScreenInstruction::ClosePane(pane_id, None, None, None)) {
        return json!({"error": format!("Failed to close pane: {}", e)});
    }

    json!({"success": true, "message": format!("Closed pane {}", pane_id_str)})
}

fn handle_new_tab(args: Value, screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    use zellij_utils::data::ClientId;

    let client_id: ClientId = 1;
    let tab_name = args.get("name").and_then(|n| n.as_str()).map(String::from);

    if let Err(e) = screen_sender.send(ScreenInstruction::NewTab(
        None,
        None,
        None,
        vec![],
        tab_name,
        (vec![], vec![]),
        None,
        false,
        false, // Don't change focus by default
        (client_id, false),
        None,
    )) {
        return json!({"error": format!("Failed to create new tab: {}", e)});
    }

    json!({"success": true, "message": "New tab created"})
}

fn handle_close_tab(screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    use zellij_utils::data::ClientId;

    let client_id: ClientId = 1;

    if let Err(e) = screen_sender.send(ScreenInstruction::CloseTab(client_id, None)) {
        return json!({"error": format!("Failed to close tab: {}", e)});
    }

    json!({"success": true, "message": "Tab closed"})
}



fn handle_query_tab_names(screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    let (response_tx, response_rx) = crossbeam::channel::bounded(1);

    if let Err(e) = screen_sender.send(ScreenInstruction::GetTabInfo {
        response_channel: response_tx,
    }) {
        return json!({"error": format!("Failed to send GetTabInfo instruction: {}", e)});
    }

    match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(tab_info) => {
            let tabs: Vec<serde_json::Value> = tab_info
                .iter()
                .map(|(index, name)| {
                    json!({
                        "index": index,
                        "name": name
                    })
                })
                .collect();

            json!({
                "success": true,
                "tabs": tabs
            })
        },
        Err(e) => {
            json!({"error": format!("Timeout waiting for tab info: {}", e)})
        },
    }
}

fn handle_rename_session(
    args: Value,
    screen_sender: &SenderWithContext<ScreenInstruction>,
) -> Value {
    use zellij_utils::data::ClientId;

    let new_name = match args.get("new_name").and_then(|n| n.as_str()) {
        Some(name) => name,
        None => return json!({"error": "Missing new_name parameter"}),
    };

    let client_id: ClientId = 1;

    if let Err(e) = screen_sender.send(ScreenInstruction::RenameSession(
        new_name.to_string(),
        client_id,
        None,
    )) {
        return json!({"error": format!("Failed to rename session: {}", e)});
    }

    json!({"success": true, "message": format!("Session renamed to '{}'", new_name)})
}

fn handle_launch_plugin(
    args: Value,
    screen_sender: &SenderWithContext<ScreenInstruction>,
) -> Value {
    use zellij_utils::cli::CliArgs;
    use zellij_utils::data::ClientId;
    use zellij_utils::input::config::Config;
    use zellij_utils::input::layout::RunPluginOrAlias;

    let url = match args.get("url").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => return json!({"error": "Missing url parameter"}),
    };

    let should_float = args
        .get("floating")
        .and_then(|f| f.as_bool())
        .unwrap_or(false);
    let client_id: ClientId = 1;

    // Load config to get plugin aliases
    let cli_args = CliArgs::default();
    let config = match Config::try_from(&cli_args) {
        Ok(config) => config,
        Err(e) => {
            return json!({"error": format!("Failed to load config: {}", e)});
        },
    };

    // Use from_url which handles both URLs and aliases
    let run_plugin_or_alias =
        match RunPluginOrAlias::from_url(url, &None, Some(&config.plugins), None) {
            Ok(plugin) => plugin,
            Err(e) => {
                return json!({"error": format!("Failed to parse plugin URL or alias: {}", e)})
            },
        };

    if let Err(e) = screen_sender.send(ScreenInstruction::LaunchPlugin(
        run_plugin_or_alias,
        should_float,
        false,
        None,
        false,
        None,
        client_id,
        None,
    )) {
        return json!({"error": format!("Failed to launch plugin: {}", e)});
    }

    json!({"success": true, "message": format!("Plugin launched: {}", url)})
}

fn handle_dump_layout(args: Value, screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    use zellij_utils::data::ClientId;

    let path = args.get("path").and_then(|p| p.as_str()).map(PathBuf::from);
    let client_id: ClientId = 1;

    if let Err(e) = screen_sender.send(ScreenInstruction::DumpLayout(path.clone(), client_id, None))
    {
        return json!({"error": format!("Failed to dump layout: {}", e)});
    }

    if let Some(p) = path {
        json!({"success": true, "message": format!("Layout dumped to {:?}", p)})
    } else {
        json!({"success": true, "message": "Layout dumped to stdout"})
    }
}

fn handle_send_keys(args: Value, screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    // send_keys bypasses bracketed-paste wrapping by using WriteRawToPaneId
    // (which calls write_to_pane_id_without_preprocessing rather than
    // adjust_input_to_terminal). This lets us drive TUI apps that interpret
    // bracketed-paste sequences as paste events rather than key presses.

    let pane_id_str = match args.get("pane_id").and_then(|p| p.as_str()) {
        Some(id) => id,
        None => return json!({"error": "Missing pane_id"}),
    };

    let pane_id = match parse_pane_id(pane_id_str) {
        Ok(id) => id,
        Err(e) => return json!({"error": e}),
    };

    let keys = match args.get("keys").and_then(|k| k.as_array()) {
        Some(k) => k.clone(),
        None => return json!({"error": "Missing keys array"}),
    };

    let inter_key_delay_ms = args
        .get("inter_key_delay_ms")
        .and_then(|d| d.as_u64())
        .unwrap_or(0);

    let mut sent = Vec::new();
    for key_val in &keys {
        let key_str = match key_val.as_str() {
            Some(s) => s,
            None => return json!({"error": "keys entries must be strings"}),
        };

        let bytes = match parse_key_string(key_str) {
            Ok(b) => b,
            Err(e) => return json!({"error": format!("Invalid key '{}': {}", key_str, e)}),
        };

        if let Err(e) = screen_sender.send(ScreenInstruction::WriteRawToPaneId(
            bytes,
            pane_id,
        )) {
            return json!({"error": format!("Failed to send key '{}': {}", key_str, e)});
        }

        sent.push(key_str);

        if inter_key_delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(inter_key_delay_ms));
        }
    }

    json!({
        "success": true,
        "message": format!("Sent {} key(s) to pane {}", sent.len(), pane_id_str),
        "keys_sent": sent
    })
}

/// Parse a key string into the ANSI/xterm byte sequence it produces.
///
/// Supported formats:
///   Named keys:   enter, return, escape, esc, tab, backspace, space,
///                 up, down, left, right, home, end, pageup, pagedown, delete,
///                 f1..f12
///   Modified:     ctrl+<char>, alt+<char>, shift+<key>, alt+enter, ctrl+enter
///   Raw hex:      hex:0d  or  hex:1b5b41  (each pair of hex digits = one byte)
///   Literal text: literal:abc  (UTF-8 sent as-is, without paste markers)
fn parse_key_string(key: &str) -> Result<Vec<u8>, String> {
    // hex:XXXXXX — raw byte sequence in hex
    if let Some(hex_str) = key.strip_prefix("hex:") {
        if hex_str.len() % 2 != 0 {
            return Err(format!("hex string '{}' has odd length", hex_str));
        }
        let mut bytes = Vec::new();
        for i in (0..hex_str.len()).step_by(2) {
            let byte = u8::from_str_radix(&hex_str[i..i + 2], 16)
                .map_err(|_| format!("invalid hex pair '{}'", &hex_str[i..i + 2]))?;
            bytes.push(byte);
        }
        return Ok(bytes);
    }

    // literal:TEXT — UTF-8 text sent verbatim
    if let Some(text) = key.strip_prefix("literal:") {
        return Ok(text.as_bytes().to_vec());
    }

    // ctrl+X
    if let Some(rest) = key.strip_prefix("ctrl+") {
        match rest {
            "enter" | "return" => return Ok(vec![0x0d]), // same as plain Enter
            "space" => return Ok(vec![0x00]),
            "[" => return Ok(vec![0x1b]),
            "\\" => return Ok(vec![0x1c]),
            "]" => return Ok(vec![0x1d]),
            "^" => return Ok(vec![0x1e]),
            "_" => return Ok(vec![0x1f]),
            s if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                let b = c as u8;
                if b.is_ascii_alphabetic() {
                    return Ok(vec![b.to_ascii_lowercase() - b'a' + 1]);
                }
                return Err(format!("ctrl+{} is not a recognized combination", s));
            },
            _ => return Err(format!("ctrl+{} is not a recognized combination", rest)),
        }
    }

    // alt+X  — ESC prefix
    if let Some(rest) = key.strip_prefix("alt+") {
        let inner = parse_key_string(rest)?;
        let mut bytes = vec![0x1b];
        bytes.extend(inner);
        return Ok(bytes);
    }

    // shift+X  — just send the named key (caller supplies shifted char literally if needed)
    if let Some(rest) = key.strip_prefix("shift+") {
        return parse_key_string(rest);
    }

    // Named keys
    match key.to_lowercase().as_str() {
        "enter" | "return" => return Ok(vec![0x0d]),
        "escape" | "esc" => return Ok(vec![0x1b]),
        "tab" => return Ok(vec![0x09]),
        "backspace" => return Ok(vec![0x7f]),
        "space" => return Ok(vec![0x20]),
        "up" => return Ok(vec![0x1b, b'[', b'A']),
        "down" => return Ok(vec![0x1b, b'[', b'B']),
        "right" => return Ok(vec![0x1b, b'[', b'C']),
        "left" => return Ok(vec![0x1b, b'[', b'D']),
        "home" => return Ok(vec![0x1b, b'[', b'H']),
        "end" => return Ok(vec![0x1b, b'[', b'F']),
        "pageup" => return Ok(vec![0x1b, b'[', b'5', b'~']),
        "pagedown" => return Ok(vec![0x1b, b'[', b'6', b'~']),
        "delete" => return Ok(vec![0x1b, b'[', b'3', b'~']),
        "f1" => return Ok(vec![0x1b, b'O', b'P']),
        "f2" => return Ok(vec![0x1b, b'O', b'Q']),
        "f3" => return Ok(vec![0x1b, b'O', b'R']),
        "f4" => return Ok(vec![0x1b, b'O', b'S']),
        "f5" => return Ok(vec![0x1b, b'[', b'1', b'5', b'~']),
        "f6" => return Ok(vec![0x1b, b'[', b'1', b'7', b'~']),
        "f7" => return Ok(vec![0x1b, b'[', b'1', b'8', b'~']),
        "f8" => return Ok(vec![0x1b, b'[', b'1', b'9', b'~']),
        "f9" => return Ok(vec![0x1b, b'[', b'2', b'0', b'~']),
        "f10" => return Ok(vec![0x1b, b'[', b'2', b'1', b'~']),
        "f11" => return Ok(vec![0x1b, b'[', b'2', b'3', b'~']),
        "f12" => return Ok(vec![0x1b, b'[', b'2', b'4', b'~']),
        _ => {},
    }

    // Single printable ASCII character — send as-is (allows alt+a → ESC+'a')
    let chars: Vec<char> = key.chars().collect();
    if chars.len() == 1 && chars[0].is_ascii() && !chars[0].is_ascii_control() {
        return Ok(vec![chars[0] as u8]);
    }

    Err(format!("Unknown key: '{}'", key))
}

fn handle_list_aliases() -> Value {
    use zellij_utils::cli::CliArgs;
    use zellij_utils::input::config::Config;

    let cli_args = CliArgs::default();
    let config = match Config::try_from(&cli_args) {
        Ok(config) => config,
        Err(e) => {
            return json!({"error": format!("Failed to load config: {}", e)});
        },
    };

    let mut aliases = Vec::new();
    for (alias_name, run_plugin) in config.plugins.aliases.iter() {
        aliases.push(json!({
            "alias": alias_name,
            "location": format!("{:?}", run_plugin.location)
        }));
    }

    json!({
        "success": true,
        "aliases": aliases
    })
}

#[cfg(test)]
mod tests {
    use super::parse_key_string;

    #[test]
    fn test_named_keys() {
        assert_eq!(parse_key_string("enter").unwrap(), vec![0x0d]);
        assert_eq!(parse_key_string("return").unwrap(), vec![0x0d]);
        assert_eq!(parse_key_string("escape").unwrap(), vec![0x1b]);
        assert_eq!(parse_key_string("esc").unwrap(), vec![0x1b]);
        assert_eq!(parse_key_string("tab").unwrap(), vec![0x09]);
        assert_eq!(parse_key_string("backspace").unwrap(), vec![0x7f]);
        assert_eq!(parse_key_string("space").unwrap(), vec![0x20]);
    }

    #[test]
    fn test_arrow_keys() {
        assert_eq!(parse_key_string("up").unwrap(), vec![0x1b, b'[', b'A']);
        assert_eq!(parse_key_string("down").unwrap(), vec![0x1b, b'[', b'B']);
        assert_eq!(parse_key_string("right").unwrap(), vec![0x1b, b'[', b'C']);
        assert_eq!(parse_key_string("left").unwrap(), vec![0x1b, b'[', b'D']);
    }

    #[test]
    fn test_function_keys() {
        assert_eq!(parse_key_string("f1").unwrap(), vec![0x1b, b'O', b'P']);
        assert_eq!(parse_key_string("f5").unwrap(), vec![0x1b, b'[', b'1', b'5', b'~']);
        assert_eq!(parse_key_string("f12").unwrap(), vec![0x1b, b'[', b'2', b'4', b'~']);
    }

    #[test]
    fn test_ctrl_keys() {
        assert_eq!(parse_key_string("ctrl+c").unwrap(), vec![0x03]);
        assert_eq!(parse_key_string("ctrl+a").unwrap(), vec![0x01]);
        assert_eq!(parse_key_string("ctrl+z").unwrap(), vec![0x1a]);
        assert_eq!(parse_key_string("ctrl+enter").unwrap(), vec![0x0d]);
    }

    #[test]
    fn test_alt_keys() {
        // alt+enter = ESC + Enter byte (0x0d)
        assert_eq!(parse_key_string("alt+enter").unwrap(), vec![0x1b, 0x0d]);
        // alt+a = ESC + 'a' (0x61)
        assert_eq!(parse_key_string("alt+a").unwrap(), vec![0x1b, 0x61]);
        // alt+escape = ESC + ESC
        assert_eq!(parse_key_string("alt+escape").unwrap(), vec![0x1b, 0x1b]);
    }

    #[test]
    fn test_hex_keys() {
        assert_eq!(parse_key_string("hex:0d").unwrap(), vec![0x0d]);
        assert_eq!(parse_key_string("hex:1b5b41").unwrap(), vec![0x1b, 0x5b, 0x41]);
    }

    #[test]
    fn test_literal_keys() {
        assert_eq!(parse_key_string("literal:hello").unwrap(), b"hello");
        assert_eq!(parse_key_string("literal:").unwrap(), b"");
    }

    #[test]
    fn test_invalid_key() {
        assert!(parse_key_string("notakey").is_err());
        assert!(parse_key_string("ctrl+xyz").is_err());
        assert!(parse_key_string("hex:0").is_err()); // odd length
        assert!(parse_key_string("hex:zz").is_err()); // invalid hex
    }
}
