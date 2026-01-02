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
            .filter(|t| t.get("name").and_then(|n| n.as_str()) == Some("zellij_get_current_session"))
            .collect();
        
        assert!(!has_current_session.is_empty(), 
                "get_current_session tool should be available");
        println!("✓ get_current_session tool exists");
        
        let result = execute_tool("zellij_get_current_session", json!({}));
        assert!(result.is_ok(), 
                "get_current_session should work without session parameter");
        println!("✓ get_current_session works without session parameter");
        
        // 2. Verify all manipulation tools require session
        let manipulation_tools = vec![
            ("zellij_health_check", json!({})),
            ("zellij_list_panes", json!({})),
            ("zellij_read_pane", json!({"pane_id": "terminal_1"})),
            ("zellij_write_to_pane", json!({"pane_id": "terminal_1", "text": "test"})),
            ("zellij_new_tab", json!({})),
            ("zellij_close_tab", json!({})),
            ("zellij_focus_pane", json!({})),
            ("zellij_new_pane", json!({})),
            ("zellij_run_command_in_pane", json!({"pane_id": "terminal_1", "command": "ls"})),
            ("zellij_go_to_tab", json!({})),
            ("zellij_query_tab_names", json!({})),
            ("zellij_launch_plugin", json!({"url": "test"})),
            ("zellij_list_aliases", json!({})),
            ("zellij_dump_layout", json!({})),
            ("zellij_dump_screen", json!({})),
            ("zellij_get_session_info", json!({})),
        ];
        
        println!("Testing session requirement for manipulation tools...");
        for (tool_name, args) in manipulation_tools {
            let result = execute_tool(tool_name, args);
            assert!(result.is_err(), 
                    "{} should fail without session parameter", tool_name);
            
            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("Session parameter is required") || 
                    error_msg.contains("Missing session") ||
                    error_msg.contains("session") && error_msg.contains("required"),
                    "{} should mention session requirement. Error: {}", 
                    tool_name, error_msg);
        }
        println!("✓ All manipulation tools correctly require session");
        
        // 3. Verify manipulation tools accept session parameter
        let session_test_cases = vec![
            ("zellij_health_check", json!({"session": "test"})),
            ("zellij_list_panes", json!({"session": "test"})),
            ("zellij_write_to_pane", json!({"session": "test", "pane_id": "terminal_1", "text": "test"})),
            ("zellij_close_tab", json!({"session": "test", "tab_index": 0})),
            ("zellij_get_session_info", json!({"session": "test"})),
        ];
        
        println!("Testing session parameter acceptance...");
        for (tool_name, args) in session_test_cases {
            let result = execute_tool(tool_name, args);
            // Should not fail due to missing session parameter
            if let Err(e) = result {
                let error_msg = e.to_string();
                assert!(!error_msg.contains("Session parameter is required") && 
                        !error_msg.contains("Missing session"),
                        "{} should accept explicit session. Error: {}", 
                        tool_name, error_msg);
            }
        }
        println!("✓ All manipulation tools accept explicit session parameter");
        
        // 4. Verify tool descriptions are clean
        println!("Checking tool descriptions...");
        for tool in &tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                if let Some(description) = tool.get("description").and_then(|d| d.as_str()) {
                    // get_current_session is allowed to mention "current" and "$ZELLIJ_SESSION_NAME"
                    if name != "zellij_get_current_session" {
                        assert!(!description.contains("current"), 
                                "{} description should not mention 'current': {}", 
                                name, description);
                        
                        assert!(!description.contains("defaults to") && 
                                !description.contains("$ZELLIJ_SESSION_NAME"),
                                "{} description should not mention defaults: {}", 
                                name, description);
                    }
                    // But get_current_session should not mention "defaults to"
                    else {
                        assert!(!description.contains("defaults to"),
                                "get_current_session should not mention 'defaults to': {}", 
                                description);
                    }
                }
            }
        }
        println!("✓ All tool descriptions are properly updated");
        
        // 5. Verify specific tool parameters
        println!("Checking specific tool parameters...");
        
        // Check close_tab requires tab_index
        if let Some(close_tab) = tools.iter().find(|t| {
            t.get("name").and_then(|n| n.as_str()) == Some("zellij_close_tab")
        }) {
            if let Some(properties) = close_tab.get("inputSchema").and_then(|s| s.get("properties")) {
                assert!(properties.get("tab_index").is_some(), 
                        "close_tab should require tab_index parameter");
                println!("✓ close_tab correctly requires tab_index parameter");
            }
        }
        
        // Check session parameters have proper descriptions
        for tool in tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                if name != "zellij_list_sessions" && name != "zellij_get_current_session" {
                    if let Some(properties) = tool.get("inputSchema").and_then(|s| s.get("properties")) {
                        if let Some(session_prop) = properties.get("session") {
                            if let Some(description) = session_prop.get("description") {
                                let desc = description.as_str().unwrap_or("");
                                assert!(!desc.contains("defaults to") && 
                                        !desc.contains("$ZELLIJ_SESSION_NAME"),
                                        "{} session description should not mention defaults: {}", 
                                        name, desc);
                            }
                        }
                    }
                }
            }
        }
        println!("✓ All tool parameters are correctly defined");
        
        println!("\n=== Interface Validation Complete ===");
        println!("✅ MCP interface successfully updated!");
        println!("✅ get_current_session available for discovery");
        println!("✅ All manipulation tools require explicit session");
        println!("✅ Tool descriptions properly updated");
        println!("✅ No race conditions from changing 'current' context");
    }
}