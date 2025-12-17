use crate::protocol::{McpError, McpRequest, McpResponse};
use crate::tools::{execute_tool, get_all_tool_definitions};
use anyhow::Result;
use std::io::{stdin, stdout, BufRead, Write};

/// Run the MCP client (stateless stdio proxy)
pub fn run_mcp_client() -> Result<()> {
    let stdin = stdin();
    let mut stdout = stdout();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Parse request
        let request: McpRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let error_response =
                    McpResponse::error(McpError::parse_error(format!("Invalid JSON: {}", e)), None);
                writeln!(stdout, "{}", serde_json::to_string(&error_response)?)?;
                stdout.flush()?;
                continue;
            },
        };

        // Handle request
        let response = handle_request(request);

        // Write response
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_request(request: McpRequest) -> McpResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request),
        "tools/list" => handle_list_tools(request),
        "tools/call" => handle_tool_call(request),
        "ping" => McpResponse::success(serde_json::json!({"pong": true}), request.id),
        _ => McpResponse::error(McpError::method_not_found(&request.method), request.id),
    }
}

fn handle_initialize(request: McpRequest) -> McpResponse {
    McpResponse::success(
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "zellij-mcp",
                "version": "0.44.0"
            }
        }),
        request.id,
    )
}

fn handle_list_tools(request: McpRequest) -> McpResponse {
    let tools = get_all_tool_definitions();
    McpResponse::success(serde_json::json!({"tools": tools}), request.id)
}

fn handle_tool_call(request: McpRequest) -> McpResponse {
    let params = match request.params.as_ref() {
        Some(p) => p,
        None => return McpResponse::error(McpError::invalid_params("Missing params"), request.id),
    };

    let tool_name = match params.get("name").and_then(|n| n.as_str()) {
        Some(name) => name,
        None => {
            return McpResponse::error(McpError::invalid_params("Missing tool name"), request.id)
        },
    };

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // Execute tool
    match execute_tool(tool_name, arguments) {
        Ok(result) => McpResponse::success(result, request.id),
        Err(e) => McpResponse::error(McpError::internal_error(format!("{}", e)), request.id),
    }
}
