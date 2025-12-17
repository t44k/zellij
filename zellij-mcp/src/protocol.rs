use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String, // "2.0"
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String, // "2.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpResponse {
    pub fn success(result: Value, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(error: McpError, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

impl McpError {
    pub fn parse_error(message: String) -> Self {
        Self {
            code: -32700,
            message,
            data: None,
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    pub fn invalid_params(msg: &str) -> Self {
        Self {
            code: -32602,
            message: msg.to_string(),
            data: None,
        }
    }

    pub fn internal_error(message: String) -> Self {
        Self {
            code: -32603,
            message,
            data: None,
        }
    }
}
