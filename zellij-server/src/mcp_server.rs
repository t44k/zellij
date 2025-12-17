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
        "focus_pane" => handle_focus_pane(args, screen_sender),
        "new_pane" => handle_new_pane(args, pty_sender),
        "close_pane" => handle_close_pane(args, screen_sender),
        "new_tab" => handle_new_tab(args, screen_sender),
        "close_tab" => handle_close_tab(screen_sender),
        "go_to_tab" => handle_go_to_tab(args, screen_sender),
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

    let client_id: ClientId = 1;

    if let Err(e) = pty_sender.send(PtyInstruction::SpawnTerminal(
        terminal_action,
        None,
        placement,
        false,
        ClientTabIndexOrPaneId::ClientId(client_id),
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
        true,
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

fn handle_go_to_tab(args: Value, screen_sender: &SenderWithContext<ScreenInstruction>) -> Value {
    use zellij_utils::data::ClientId;

    let client_id: ClientId = 1;

    if let Some(index) = args.get("index").and_then(|i| i.as_u64()) {
        // MCP accepts 0-based indices (matching query_tab_names output)
        // but Screen::go_to_tab expects 1-based, so we add 1
        // Use checked_add to prevent overflow on extreme values
        let tab_index = match index.checked_add(1) {
            Some(idx) if idx <= u32::MAX as u64 => idx as u32,
            _ => {
                return json!({"error": format!("Tab index {} is out of valid range (0-{})", index, u32::MAX - 1)});
            }
        };

        if let Err(e) =
            screen_sender.send(ScreenInstruction::GoToTab(tab_index, Some(client_id), None))
        {
            return json!({"error": format!("Failed to go to tab: {}", e)});
        }

        json!({"success": true, "message": format!("Switched to tab {}", index)})
    } else if let Some(name) = args.get("name").and_then(|n| n.as_str()) {
        if let Err(e) = screen_sender.send(ScreenInstruction::GoToTabName(
            name.to_string(),
            (vec![], vec![]),
            None,
            false,
            Some(client_id),
            None,
        )) {
            return json!({"error": format!("Failed to go to tab: {}", e)});
        }

        json!({"success": true, "message": format!("Switched to tab '{}'", name)})
    } else {
        json!({"error": "Must provide either 'index' or 'name' parameter"})
    }
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
