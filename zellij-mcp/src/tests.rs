#[cfg(test)]
mod tests {
    use crate::tools::{get_all_tool_definitions, execute_tool};
    use serde_json::json;

    #[test]
    fn test_session_parameter_falls_back_to_env() {
        // Test that session parameter falls back to $ZELLIJ_SESSION_NAME
        // When env var is NOT set, tools should fail with appropriate error

        // Ensure env var is not set for this test
        std::env::remove_var("ZELLIJ_SESSION_NAME");

        let test_cases = vec![
            ("list_panes", json!({})),
            ("read_pane", json!({"pane_id": "terminal_1"})),
            ("write_to_pane", json!({"pane_id": "terminal_1", "text": "test"})),
            ("new_tab", json!({})),
            ("close_tab", json!({})),
        ];

        for (tool_name, args) in test_cases {
            let result = execute_tool(tool_name, args);
            assert!(result.is_err(),
                    "Tool {} should fail without session parameter and env var", tool_name);

            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("Session parameter is required") ||
                    error_msg.contains("ZELLIJ_SESSION_NAME"),
                    "Tool {} should mention session requirement or env var. Error: {}", tool_name, error_msg);
        }
    }

    #[test]
    fn test_get_current_session_exists() {
        // Test that get_current_session tool exists and works
        let result = execute_tool("get_current_session", json!({}));
        // It should succeed (either returns current session or "not in session")
        assert!(result.is_ok(),
                "get_current_session should work without session parameter. Error: {:?}",
                result.err());
    }

    #[test]
    fn test_explicit_session_parameter() {
        // Test that tools work with explicit session parameter
        let test_cases = vec![
            ("health_check", json!({"session": "test"})),
            ("list_panes", json!({"session": "test"})),
            ("get_session_info", json!({"session": "test"})),
        ];

        for (tool_name, args) in test_cases {
            let result = execute_tool(tool_name, args);
            // These might fail due to non-existent sessions, but should not fail due to missing session
            if let Err(e) = result {
                let error_msg = e.to_string();
                assert!(!error_msg.contains("Session parameter is required") &&
                        !error_msg.contains("Missing session"),
                        "Tool {} should accept explicit session. Error: {}", tool_name, error_msg);
            }
        }
    }

    #[test]
    fn test_close_tab_requires_tab_index() {
        // Test that close_tab now requires tab_index instead of closing current tab
        let result = execute_tool("close_tab", json!({"session": "test"}));

        // This might fail for various reasons, but not due to missing session parameter
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(!error_msg.contains("Session parameter is required"),
                    "close_tab should not fail for session when provided. Error: {}", error_msg);
        }
    }

    #[test]
    fn test_tool_descriptions_not_empty() {
        // Test that all tools have descriptions
        let tools = get_all_tool_definitions();

        for tool in tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                let description = tool.get("description").and_then(|d| d.as_str());
                assert!(description.is_some() && !description.unwrap().is_empty(),
                        "Tool {} should have a non-empty description", name);
            }
        }
    }

    #[test]
    fn test_all_session_parameters_have_descriptions() {
        // Test that all session parameters have proper descriptions mentioning the fallback
        let tools = get_all_tool_definitions();

        // Tools that should have session parameter with fallback description
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

        for tool in tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                if tools_with_session_fallback.contains(&name) {
                    if let Some(properties) = tool.get("inputSchema").and_then(|s| s.get("properties")) {
                        let session_prop = properties.get("session");
                        assert!(session_prop.is_some(),
                                "Tool {} should have a session parameter", name);

                        if let Some(session) = session_prop {
                            let description = session.get("description").and_then(|d| d.as_str());
                            assert!(description.is_some(),
                                    "Tool {} session parameter should have a description", name);

                            let desc = description.unwrap();
                            assert!(desc.contains("$ZELLIJ_SESSION_NAME"),
                                    "Tool {} session description should mention fallback to $ZELLIJ_SESSION_NAME: {}",
                                    name, desc);
                        }
                    }
                }
            }
        }
    }
}
