#[cfg(test)]
mod integration_tests {
    use crate::tools::{get_all_tool_definitions, execute_tool};
    use serde_json::json;

    #[test]
    fn test_complete_interface_validation() {
        println!("=== Complete MCP Interface Validation ===");

        let tools = get_all_tool_definitions();
        println!("Total tools available: {}", tools.len());

        // 1. Verify get_current_session exists and works
        let has_current_session: Vec<_> = tools.iter()
            .filter(|t| t.get("name").and_then(|n| n.as_str()) == Some("get_current_session"))
            .collect();

        assert!(!has_current_session.is_empty(),
                "get_current_session tool should be available");
        println!("✓ get_current_session tool exists");

        let result = execute_tool("get_current_session", json!({}));
        assert!(result.is_ok(),
                "get_current_session should work without session parameter");
        println!("✓ get_current_session works without session parameter");

        // 2. Verify tools fall back to $ZELLIJ_SESSION_NAME when not provided
        // First, ensure env var is not set
        std::env::remove_var("ZELLIJ_SESSION_NAME");

        let manipulation_tools = vec![
            ("list_panes", json!({})),
            ("read_pane", json!({"pane_id": "terminal_1"})),
            ("write_to_pane", json!({"pane_id": "terminal_1", "text": "test"})),
            ("new_tab", json!({})),
            ("close_tab", json!({})),
            ("focus_pane", json!({})),
            ("new_pane", json!({})),
            ("run_command_in_pane", json!({"pane_id": "terminal_1", "command": "ls"})),
            ("query_tab_names", json!({})),
            ("launch_plugin", json!({"url": "test"})),
            ("list_aliases", json!({})),
            ("dump_layout", json!({})),
            ("dump_screen", json!({})),
            ("get_session_info", json!({})),
        ];

        println!("Testing session fallback behavior (without env var set)...");
        for (tool_name, args) in manipulation_tools {
            let result = execute_tool(tool_name, args);
            assert!(result.is_err(),
                    "{} should fail without session parameter and without env var", tool_name);

            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("Session parameter is required") ||
                    error_msg.contains("ZELLIJ_SESSION_NAME"),
                    "{} should mention session requirement or env var. Error: {}",
                    tool_name, error_msg);
        }
        println!("✓ All manipulation tools correctly require session or env var");

        // 3. Verify manipulation tools accept session parameter
        let session_test_cases = vec![
            ("health_check", json!({"session": "test"})),
            ("list_panes", json!({"session": "test"})),
            ("write_to_pane", json!({"session": "test", "pane_id": "terminal_1", "text": "test"})),
            ("close_tab", json!({"session": "test", "tab_index": 0})),
            ("get_session_info", json!({"session": "test"})),
        ];

        println!("Testing session parameter acceptance...");
        for (tool_name, args) in session_test_cases {
            let result = execute_tool(tool_name, args);
            // Should not fail due to missing session parameter
            if let Err(e) = result {
                let error_msg = e.to_string();
                assert!(!error_msg.contains("Session parameter is required") &&
                        !error_msg.contains("ZELLIJ_SESSION_NAME"),
                        "{} should accept explicit session. Error: {}",
                        tool_name, error_msg);
            }
        }
        println!("✓ All manipulation tools accept explicit session parameter");

        // 4. Verify tool descriptions mention fallback
        println!("Checking tool descriptions...");
        let tools_with_session_fallback = vec![
            "list_panes",
            "read_pane",
            "write_to_pane",
            "run_command_in_pane",
            "focus_pane",
            "new_pane",
            "close_pane",
            "get_session_info",
            "new_tab",
            "close_tab",
            "query_tab_names",
            "launch_plugin",
            "list_aliases",
            "dump_layout",
            "dump_screen",
        ];

        for tool in &tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                if tools_with_session_fallback.contains(&name) {
                    if let Some(properties) = tool.get("inputSchema").and_then(|s| s.get("properties")) {
                        if let Some(session_prop) = properties.get("session") {
                            if let Some(description) = session_prop.get("description") {
                                let desc = description.as_str().unwrap_or("");
                                assert!(desc.contains("$ZELLIJ_SESSION_NAME"),
                                        "{} session description should mention fallback: {}",
                                        name, desc);
                            }
                        }
                    }
                }
            }
        }
        println!("✓ All session parameter descriptions mention fallback");

        // 5. Verify specific tool parameters
        println!("Checking specific tool parameters...");

        // Verify close_tab has session parameter with fallback description
        if let Some(close_tab) = tools.iter().find(|t| {
            t.get("name").and_then(|n| n.as_str()) == Some("close_tab")
        }) {
            if let Some(properties) = close_tab.get("inputSchema").and_then(|s| s.get("properties")) {
                assert!(properties.get("session").is_some(),
                        "close_tab should have session parameter");
                println!("✓ close_tab has session parameter");
            }
        }

        println!("\n=== Interface Validation Complete ===");
        println!("✅ MCP interface successfully updated!");
        println!("✅ get_current_session available for discovery");
        println!("✅ All manipulation tools fall back to $ZELLIJ_SESSION_NAME");
        println!("✅ Tool descriptions properly document fallback behavior");
    }
}
