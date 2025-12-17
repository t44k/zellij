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
            "zellij_list_sessions",
            "List all active Zellij sessions with MCP status",
            json!({}),
        ),
        tool_def(
            "zellij_get_current_session",
            "Get the current session from environment ($ZELLIJ_SESSION_NAME)",
            json!({}),
        ),
        tool_def(
            "zellij_health_check",
            "Check MCP server health and configuration",
            json!({
                "session": {"type": "string", "description": "Optional session to check (defaults to current)"}
            }),
        ),
        // Pane operations (7)
        tool_def(
            "zellij_list_panes",
            "List all panes in the session with metadata",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"}
            }),
        ),
        tool_def(
            "zellij_read_pane",
            "Read pane content with optional scrollback",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "pane_id": {"type": "string", "description": "Pane ID (e.g., 'terminal_1', 'plugin_2')"},
                "include_scrollback": {"type": "boolean", "description": "Include scrollback history (default: false)"},
                "lines": {"type": "number", "description": "Maximum lines to return"}
            }),
        ),
        tool_def(
            "zellij_write_to_pane",
            "Write text to a specific pane without focusing it",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "pane_id": {"type": "string", "description": "Pane ID"},
                "text": {"type": "string", "description": "Text to write"},
                "submit": {"type": "boolean", "description": "Submit with Enter key (default: false)"}
            }),
        ),
        tool_def(
            "zellij_run_command_in_pane",
            "Execute a command in a specific pane without focusing it",
            json!({
                "session": {"type": "string", "description": "Session name (defaults to $ZELLIJ_SESSION_NAME)"},
                "pane_id": {"type": "string", "description": "Pane ID"},
                "command": {"type": "string", "description": "Command to execute"}
            }),
        ),
        tool_def(
            "zellij_focus_pane",
            "Focus a specific pane",
            json!({
                "session": {"type": "string"},
                "pane_id": {"type": "string", "description": "Pane ID"}
            }),
        ),
        tool_def(
            "zellij_new_pane",
            "Create a new pane",
            json!({
                "session": {"type": "string"},
                "direction": {"type": "string", "description": "Split direction: 'right', 'down', 'left', 'up'"},
                "command": {"type": "string", "description": "Command to run in new pane"},
                "cwd": {"type": "string", "description": "Working directory"}
            }),
        ),
        tool_def(
            "zellij_close_pane",
            "Close a specific pane or focused pane",
            json!({
                "session": {"type": "string"},
                "pane_id": {"type": "string", "description": "Pane ID (omit to close focused pane)"}
            }),
        ),
        // Session management (4)
        tool_def(
            "zellij_attach_session",
            "Get command to attach to a session",
            json!({
                "session_name": {"type": "string", "description": "Session to attach to"}
            }),
        ),
        tool_def(
            "zellij_new_session",
            "Create a new Zellij session",
            json!({
                "session_name": {"type": "string", "description": "Name for new session"},
                "layout": {"type": "string", "description": "Layout to use"}
            }),
        ),
        tool_def(
            "zellij_kill_session",
            "Kill a Zellij session",
            json!({
                "session_name": {"type": "string", "description": "Session to kill"}
            }),
        ),
        tool_def(
            "zellij_get_session_info",
            "Get comprehensive session information",
            json!({
                "session": {"type": "string"}
            }),
        ),
        // Tab management (4)
        tool_def(
            "zellij_new_tab",
            "Create a new tab",
            json!({
                "session": {"type": "string"},
                "name": {"type": "string", "description": "Tab name"},
                "layout": {"type": "string", "description": "Layout for tab"}
            }),
        ),
        tool_def(
            "zellij_close_tab",
            "Close the current tab",
            json!({
                "session": {"type": "string"}
            }),
        ),
        tool_def(
            "zellij_go_to_tab",
            "Switch to a specific tab by index or name",
            json!({
                "session": {"type": "string"},
                "index": {"type": "number", "description": "Tab index (1-based)"},
                "name": {"type": "string", "description": "Tab name"}
            }),
        ),
        tool_def(
            "zellij_query_tab_names",
            "List all tab names in the session",
            json!({
                "session": {"type": "string"}
            }),
        ),
        // Plugin & layout (3)
        tool_def(
            "zellij_launch_plugin",
            "Launch a Zellij plugin",
            json!({
                "session": {"type": "string"},
                "url": {"type": "string", "description": "Plugin URL or alias"},
                "floating": {"type": "boolean", "description": "Launch as floating pane"}
            }),
        ),
        tool_def(
            "zellij_list_aliases",
            "List available plugin aliases from config",
            json!({
                "session": {"type": "string"}
            }),
        ),
        tool_def(
            "zellij_dump_layout",
            "Export current session layout as KDL",
            json!({
                "session": {"type": "string"},
                "path": {"type": "string", "description": "Optional file path to save layout"}
            }),
        ),
        // Utility (2)
        tool_def(
            "zellij_rename_session",
            "Rename a session",
            json!({
                "session": {"type": "string", "description": "Current session name"},
                "new_name": {"type": "string", "description": "New session name"}
            }),
        ),
        tool_def(
            "zellij_dump_screen",
            "Dump the current screen to a file",
            json!({
                "session": {"type": "string"},
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
        "zellij_list_sessions" => discovery::list_sessions(),
        "zellij_get_current_session" => discovery::get_current_session(),
        "zellij_health_check" => discovery::health_check(args),

        // Pane operations
        "zellij_list_panes" => {
            let session = get_session_from_args(&args)?;
            panes::list_panes(&session)
        },
        "zellij_read_pane" => {
            let session = get_session_from_args(&args)?;
            panes::read_pane(&session, args)
        },
        "zellij_write_to_pane" => {
            let session = get_session_from_args(&args)?;
            panes::write_to_pane(&session, args)
        },
        "zellij_run_command_in_pane" => {
            let session = get_session_from_args(&args)?;
            panes::run_command_in_pane(&session, args)
        },
        "zellij_focus_pane" => {
            let session = get_session_from_args(&args)?;
            panes::focus_pane(&session, args)
        },
        "zellij_new_pane" => {
            let session = get_session_from_args(&args)?;
            panes::new_pane(&session, args)
        },
        "zellij_close_pane" => {
            let session = get_session_from_args(&args)?;
            panes::close_pane(&session, args)
        },

        // Session management
        "zellij_attach_session" => sessions::attach_session(args),
        "zellij_new_session" => sessions::new_session(args),
        "zellij_kill_session" => sessions::kill_session(args),
        "zellij_get_session_info" => {
            let session = get_session_from_args(&args)?;
            sessions::get_session_info(&session)
        },

        // Tab management
        "zellij_new_tab" => {
            let session = get_session_from_args(&args)?;
            tabs::new_tab(&session, args)
        },
        "zellij_close_tab" => {
            let session = get_session_from_args(&args)?;
            tabs::close_tab(&session)
        },
        "zellij_go_to_tab" => {
            let session = get_session_from_args(&args)?;
            tabs::go_to_tab(&session, args)
        },
        "zellij_query_tab_names" => {
            let session = get_session_from_args(&args)?;
            tabs::query_tab_names(&session)
        },

        // Plugin & layout
        "zellij_launch_plugin" => {
            let session = get_session_from_args(&args)?;
            plugins::launch_plugin(&session, args)
        },
        "zellij_list_aliases" => {
            let session = get_session_from_args(&args)?;
            plugins::list_aliases(&session)
        },
        "zellij_dump_layout" => {
            let session = get_session_from_args(&args)?;
            plugins::dump_layout(&session, args)
        },

        // Utility
        "zellij_rename_session" => {
            let session = get_session_from_args(&args)?;
            utils::rename_session(&session, args)
        },
        "zellij_dump_screen" => {
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
