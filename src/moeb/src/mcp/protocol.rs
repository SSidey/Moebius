use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(flatten)]
    pub outcome: JsonRpcOutcome,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum JsonRpcOutcome {
    Result { result: serde_json::Value },
    Error { error: JsonRpcError },
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct ToolsCapability {}

#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ListToolsResult {
    pub tools: Vec<ToolInfo>,
}

#[derive(Debug, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    pub arguments: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct TextContent {
    pub r#type: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct CallToolResult {
    pub content: Vec<TextContent>,
}
