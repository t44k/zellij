pub mod discovery;
pub mod panes;
pub mod plugins;
pub mod sessions;
pub mod tabs;
pub mod utils;

use crate::types::{get_session_from_args, tool_def};
use anyhow::Result;
use serde_json::{json, Value};

/// Get all tool definitions
pub fn get_all_tool_definitions() -> Vec<Value> {
    vec![
        // Discovery tools (3)
        tool_def(
            "list_sessions",
            "List all active Zellij sessions with MCP status",
            json!({}),
        ),
        tool_def(
            "get_current_session",
            "Get the current session from environment ($ZELLIJ_SESSION_NAME)",
            json!({}),
        ),
        tool_def(
            "health_check",
            "Check MCP server health and configuration",
            json!({
                "session": {"type": "string", "description": "Optional session to check (defaults to current)"}
            }),
        ),
        // Pane operations (7)
        tool_def(
            "list_panes",
            "List all panes in session with metadata",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "include_tab_names": {"type": "boolean", "description": "Include tab names in response for better identification (default: false)"}
            }),
        ),
        tool_def(
            "read_pane",
            "Read pane content with optional scrollback",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "tab_index": {"type": "number", "description": "Tab index (0-based). Required to avoid ambiguity between tabs."},
                "pane_id": {"type": "string", "description": "Pane ID (e.g., 'terminal_1', 'plugin_2')"},
                "include_scrollback": {"type": "boolean", "description": "Include scrollback history (default: false)"},
                "lines": {"type": "number", "description": "Maximum lines to return"},
                "offset": {"type": "number", "description": "Line offset to start reading from (default: 0)"}
            }),
        ),
        tool_def(
            "send_keys",
            "Send raw key events to a pane without bracketed-paste wrapping. Use this to drive TUI apps (press Enter, Escape, arrow keys, etc.) where write_to_pane would be interpreted as a paste event.",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "tab_index": {"type": "number", "description": "Tab index (0-based). Required to avoid ambiguity between tabs."},
                "pane_id": {"type": "string", "description": "Pane ID (e.g., 'terminal_0')"},
                "keys": {
                    "type": "array",
                    "description": "Key sequences to send. Each entry is one of: named key (enter, return, escape, esc, tab, backspace, space, up, down, left, right, home, end, pageup, pagedown, delete, f1..f12), modified key (ctrl+c, alt+enter, shift+up, etc.), raw hex bytes (hex:0d), or literal text (literal:abc).",
                    "items": {"type": "string"}
                },
                "inter_key_delay_ms": {"type": "number", "description": "Delay in milliseconds between key events (default: 0)"}
            }),
        ),
        tool_def(
            "write_to_pane",
            "Write text to a specific pane without focusing it",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "tab_index": {"type": "number", "description": "Tab index (0-based). Required to avoid ambiguity between tabs."},
                "pane_id": {"type": "string", "description": "Pane ID"},
                "text": {"type": "string", "description": "Text to write"},
                "submit": {"type": "boolean", "description": "Submit with Enter key (default: false)"}
            }),
        ),
        tool_def(
            "run_command_in_pane",
            "Execute a command in a specific pane without focusing it",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "tab_index": {"type": "number", "description": "Tab index (0-based). Required to avoid ambiguity between tabs."},
                "pane_id": {"type": "string", "description": "Pane ID"},
                "command": {"type": "string", "description": "Command to execute"}
            }),
        ),
        tool_def(
            "focus_pane",
            "Focus a specific pane",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "tab_index": {"type": "number", "description": "Tab index (0-based). Required to avoid ambiguity between tabs."},
                "pane_id": {"type": "string", "description": "Pane ID"}
            }),
        ),
        tool_def(
            "new_pane",
            "Create a new pane",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "tab_index": {"type": "number", "description": "Tab index (0-based) where to create pane. Required to avoid ambiguity."},
                "direction": {"type": "string", "description": "Split direction: 'right', 'down', 'left', 'up'"},
                "command": {"type": "string", "description": "Command to run in new pane"},
                "cwd": {"type": "string", "description": "Working directory"}
            }),
        ),
        tool_def(
            "close_pane",
            "Close a specific pane",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "tab_index": {"type": "number", "description": "Tab index (0-based). Required to avoid ambiguity between tabs."},
                "pane_id": {"type": "string", "description": "Pane ID to close"}
            }),
        ),
        // Session management (4)
        tool_def(
            "attach_session",
            "Get command to attach to a session",
            json!({
                "session_name": {"type": "string", "description": "Session to attach to"}
            }),
        ),
        tool_def(
            "new_session",
            "Create a new Zellij session",
            json!({
                "session_name": {"type": "string", "description": "Name for new session"},
                "layout": {"type": "string", "description": "Layout to use"}
            }),
        ),
        tool_def(
            "kill_session",
            "Kill a Zellij session",
            json!({
                "session_name": {"type": "string", "description": "Session to kill"}
            }),
        ),
        tool_def(
            "get_session_info",
            "Get comprehensive session information",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"}
            }),
        ),
        // Tab management (4)
        tool_def(
            "new_tab",
            "Create a new tab",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "name": {"type": "string", "description": "Tab name"},
                "layout": {"type": "string", "description": "Layout for tab"}
            }),
        ),
        tool_def(
            "close_tab",
            "Close the current tab",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"}
            }),
        ),
        
        tool_def(
            "query_tab_names",
            "List all tab names in the session with 0-based indices",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"}
            }),
        ),
        // Plugin & layout (3)
        tool_def(
            "launch_plugin",
            "Launch a Zellij plugin",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "url": {"type": "string", "description": "Plugin URL or alias"},
                "floating": {"type": "boolean", "description": "Launch as floating pane"}
            }),
        ),
        tool_def(
            "list_aliases",
            "List available plugin aliases from config",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"}
            }),
        ),
        tool_def(
            "dump_layout",
            "Export current session layout as KDL",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "path": {"type": "string", "description": "Optional file path to save layout"}
            }),
        ),
        // Utility (2)
        tool_def(
            "rename_session",
            "Rename a session",
            json!({
                "session": {"type": "string", "description": "Current session name"},
                "new_name": {"type": "string", "description": "New session name"}
            }),
        ),
        tool_def(
            "dump_screen",
            "Dump the current screen to a file",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "path": {"type": "string", "description": "File path to dump to"},
                "full": {"type": "boolean", "description": "Include full scrollback"}
            }),
        ),
    ]
}

/// Execute a tool
pub fn execute_tool(name: &str, args: Value) -> Result<Value> {
    match name {
        // Discovery tools
        "list_sessions" => discovery::list_sessions(),
        "get_current_session" => discovery::get_current_session(),
        "health_check" => discovery::health_check(args),

        // Pane operations
        "list_panes" => {
            let session = get_session_from_args(&args)?;
            panes::list_panes(&session)
        },
        "read_pane" => {
            let session = get_session_from_args(&args)?;
            panes::read_pane(&session, args)
        },
        "send_keys" => {
            let session = get_session_from_args(&args)?;
            panes::send_keys(&session, args)
        },
        "write_to_pane" => {
            let session = get_session_from_args(&args)?;
            panes::write_to_pane(&session, args)
        },
        "run_command_in_pane" => {
            let session = get_session_from_args(&args)?;
            panes::run_command_in_pane(&session, args)
        },
        "focus_pane" => {
            let session = get_session_from_args(&args)?;
            panes::focus_pane(&session, args)
        },
        "new_pane" => {
            let session = get_session_from_args(&args)?;
            panes::new_pane(&session, args)
        },
        "close_pane" => {
            let session = get_session_from_args(&args)?;
            panes::close_pane(&session, args)
        },

        // Session management
        "attach_session" => sessions::attach_session(args),
        "new_session" => sessions::new_session(args),
        "kill_session" => sessions::kill_session(args),
        "get_session_info" => {
            let session = get_session_from_args(&args)?;
            sessions::get_session_info(&session)
        },

        // Tab management
        "new_tab" => {
            let session = get_session_from_args(&args)?;
            tabs::new_tab(&session, args)
        },
        "close_tab" => {
            let session = get_session_from_args(&args)?;
            tabs::close_tab(&session, args)
        },
        
        "query_tab_names" => {
            let session = get_session_from_args(&args)?;
            tabs::query_tab_names(&session)
        },

        // Plugin & layout
        "launch_plugin" => {
            let session = get_session_from_args(&args)?;
            plugins::launch_plugin(&session, args)
        },
        "list_aliases" => {
            let session = get_session_from_args(&args)?;
            plugins::list_aliases(&session)
        },
        "dump_layout" => {
            let session = get_session_from_args(&args)?;
            plugins::dump_layout(&session, args)
        },

        // Utility
        "rename_session" => {
            let session = get_session_from_args(&args)?;
            utils::rename_session(&session, args)
        },
        "dump_screen" => {
            let session = get_session_from_args(&args)?;
            utils::dump_screen(&session, args)
        },

        _ => anyhow::bail!("Unknown tool: {}", name),
    }
}

/// Get tools list as JSON string
pub fn list_tools_json() -> String {
    serde_json::to_string_pretty(&get_all_tool_definitions()).unwrap()
}
