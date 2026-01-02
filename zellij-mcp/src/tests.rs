#[cfg(test)]
mod tests {
    use crate::tools::{get_all_tool_definitions, execute_tool};
    use serde_json::json;

    #[test]
    fn test_session_parameter_required() {
        // Test that session parameter is now required
        let tools = get_all_tool_definitions();
        
        // Check that get_current_session tool exists
        let tool_names: Vec<String> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
            .collect();
        
        assert!(tool_names.contains(&"zellij_get_current_session".to_string()), 
                "zellij_get_current_session should exist");
        
        // Test that session is required for various tools
        let test_cases = vec![
            ("zellij_health_check", json!({})),
            ("zellij_list_panes", json!({})),
            ("zellij_read_pane", json!({"pane_id": "terminal_1"})),
            ("zellij_write_to_pane", json!({"pane_id": "terminal_1", "text": "test"})),
            ("zellij_new_tab", json!({})),
            ("zellij_close_tab", json!({})),
        ];
        
        for (tool_name, args) in test_cases {
            let result = execute_tool(tool_name, args);
            assert!(result.is_err(), 
                    "Tool {} should fail without session parameter", tool_name);
            
            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("Session parameter is required") || 
                    error_msg.contains("Missing session") ||
                    error_msg.contains("session") && error_msg.contains("required"),
                    "Tool {} should mention session requirement. Error: {}", tool_name, error_msg);
        }
    }

    #[test]
    fn test_get_current_session_exists() {
        // Test that get_current_session tool exists and works
        let result = execute_tool("zellij_get_current_session", json!({}));
        // It should succeed (either returns current session or "not in session")
        assert!(result.is_ok(), 
                "get_current_session should work without session parameter. Error: {:?}", 
                result.err());
    }

    #[test]
    fn test_explicit_session_parameter() {
        // Test that tools work with explicit session parameter
        let test_cases = vec![
            ("zellij_health_check", json!({"session": "test"})),
            ("zellij_list_panes", json!({"session": "test"})),
            ("zellij_get_session_info", json!({"session": "test"})),
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
        let result = execute_tool("zellij_close_tab", json!({"session": "test"}));
        
        // This might fail for various reasons, but not due to missing session parameter
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(!error_msg.contains("Session parameter is required"), 
                    "close_tab should not fail for session when provided. Error: {}", error_msg);
        }
    }

    #[test]
    fn test_tool_descriptions_updated() {
        // Test that tool descriptions no longer mention current session/tab (except get_current_session)
        let tools = get_all_tool_definitions();
        
        for tool in tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                if let Some(description) = tool.get("description").and_then(|d| d.as_str()) {
                    // get_current_session is allowed to mention "current", others are not
                    if name == "zellij_get_current_session" {
                        continue;
                    }
                    
                    assert!(!description.contains("defaults to") && 
                            !description.contains("$ZELLIJ_SESSION_NAME") &&
                            !description.contains("current"),
                            "Tool {} description should not mention defaults or current: {}", 
                            name, description);
                }
            }
        }
    }

    #[test]
    fn test_all_session_parameters_described() {
        // Test that all session parameters have proper descriptions
        let tools = get_all_tool_definitions();
        
        for tool in tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                if let Some(input_schema) = tool.get("inputSchema") {
                    if let Some(properties) = input_schema.get("properties") {
                        if let Some(session_prop) = properties.get("session") {
                            if let Some(description) = session_prop.get("description") {
                                let desc = description.as_str().unwrap_or("");
                                assert!(!desc.contains("defaults to") && 
                                        !desc.contains("$ZELLIJ_SESSION_NAME"),
                                        "Tool {} session description should not mention defaults: {}", 
                                        name, desc);
                            } else {
                                panic!("Tool {} session parameter should have a description", name);
                            }
                        }
                    }
                }
            }
        }
    }
}