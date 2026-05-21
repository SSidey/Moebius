use std::io::{BufRead, Write};

use anyhow::Result;
use serde_json::json;

use crate::ports::ToolExecutorPort;
use crate::tools::RealToolExecutor;
use super::protocol::{
    CallToolParams, CallToolResult, InitializeResult, JsonRpcError, JsonRpcOutcome,
    JsonRpcRequest, JsonRpcResponse, ListToolsResult, ServerCapabilities, ServerInfo,
    TextContent, ToolInfo, ToolsCapability,
};
use super::session::McpSession;

// ── Shared free functions (used by both stdio and HTTP transports) ────────────

pub fn handle_initialize(id: serde_json::Value) -> JsonRpcResponse {
    let result = InitializeResult {
        protocol_version: "2024-11-05".to_string(),
        server_info: ServerInfo {
            name: "moeb".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        capabilities: ServerCapabilities {
            tools: ToolsCapability {},
        },
    };
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        outcome: JsonRpcOutcome::Result {
            result: serde_json::to_value(result).unwrap_or(json!({})),
        },
    }
}

pub fn handle_list_tools(id: serde_json::Value, executor: &RealToolExecutor) -> JsonRpcResponse {
    let tools: Vec<ToolInfo> = executor
        .registry
        .definitions()
        .into_iter()
        .map(|def| ToolInfo {
            name: def.name.to_string(),
            description: def.description.to_string(),
            input_schema: def.parameters,
        })
        .collect();
    let result = ListToolsResult { tools };
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        outcome: JsonRpcOutcome::Result {
            result: serde_json::to_value(result).unwrap_or(json!({})),
        },
    }
}

pub fn handle_call_tool(
    id: serde_json::Value,
    params: Option<serde_json::Value>,
    session: &mut McpSession,
    executor: &RealToolExecutor,
) -> JsonRpcResponse {
    let text = match do_call_tool(params, session, executor) {
        Ok(t) => t,
        Err(e) => e.to_string(),
    };
    make_content_response(id, text)
}

pub fn dispatch_request(
    request: &JsonRpcRequest,
    session: &mut McpSession,
    executor: &mut RealToolExecutor,
) -> JsonRpcResponse {
    let id = match &request.id {
        Some(id) => id.clone(),
        None => return make_content_response(json!(null), "notification ignored".to_string()),
    };
    match request.method.as_str() {
        "initialize" => handle_initialize(id),
        "tools/list" => handle_list_tools(id, executor),
        "tools/call" => handle_call_tool(id, request.params.clone(), session, executor),
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            outcome: JsonRpcOutcome::Error {
                error: JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string(),
                },
            },
        },
    }
}

fn do_call_tool(
    params: Option<serde_json::Value>,
    session: &mut McpSession,
    executor: &RealToolExecutor,
) -> Result<String> {
    let params_value = params.ok_or_else(|| anyhow::anyhow!("tools/call: missing params"))?;
    let call_params: CallToolParams = serde_json::from_value(params_value)
        .map_err(|e| anyhow::anyhow!("tools/call: invalid params: {}", e))?;
    let name = call_params.name;
    let args = call_params.arguments.unwrap_or(json!({}));
    let turn = session.increment_turn();
    let working_dir = session.working_dir.clone();
    let (text, _) = executor.execute(&name, &name, &args, &working_dir, turn)?;
    Ok(text)
}

pub fn make_content_response(id: serde_json::Value, text: String) -> JsonRpcResponse {
    let result = CallToolResult {
        content: vec![TextContent {
            r#type: "text".to_string(),
            text,
        }],
    };
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        outcome: JsonRpcOutcome::Result {
            result: serde_json::to_value(result).unwrap_or(json!({})),
        },
    }
}

// ── McpServer (stdio transport) ───────────────────────────────────────────────

pub struct McpServer {
    session: McpSession,
    executor: RealToolExecutor,
}

impl McpServer {
    pub fn new(session: McpSession, executor: RealToolExecutor) -> Self {
        Self { session, executor }
    }

    pub fn run(&mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        let mut out = stdout.lock();

        for line in stdin.lock().lines() {
            let line = line.map_err(|e| anyhow::anyhow!("stdin read error: {}", e))?;
            if line.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: json!(null),
                        outcome: JsonRpcOutcome::Error {
                            error: JsonRpcError {
                                code: -32700,
                                message: format!("Parse error: {}", e),
                            },
                        },
                    };
                    writeln!(out, "{}", serde_json::to_string(&response)?)?;
                    out.flush()?;
                    continue;
                }
            };

            if request.method == "notifications/initialized" {
                continue;
            }
            if request.id.is_none() {
                continue;
            }

            let response = dispatch_request(&request, &mut self.session, &mut self.executor);
            writeln!(out, "{}", serde_json::to_string(&response)?)?;
            out.flush()?;
        }

        Ok(())
    }

}

#[cfg(test)]
impl McpServer {
    pub(crate) fn handle_initialize(&self, id: serde_json::Value) -> JsonRpcResponse {
        handle_initialize(id)
    }

    pub(crate) fn handle_list_tools(&self, id: serde_json::Value) -> JsonRpcResponse {
        handle_list_tools(id, &self.executor)
    }

    pub(crate) fn handle_call_tool(
        &mut self,
        id: serde_json::Value,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        handle_call_tool(id, params, &mut self.session, &self.executor)
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
