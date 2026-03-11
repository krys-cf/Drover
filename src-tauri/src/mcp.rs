use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    REQUEST_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpServerConfig {
    pub name: String,
    pub server_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_command: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpToolDef {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpToolResult {
    pub content: Vec<McpContentItem>,
    pub is_error: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpContentItem {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpServerInfo {
    pub name: String,
    pub version: Option<String>,
    pub protocol_version: Option<String>,
    pub capabilities: Option<Value>,
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

fn build_jsonrpc(method: &str, params: Option<Value>) -> Value {
    let mut msg = json!({
        "jsonrpc": "2.0",
        "method": method,
    });
    if method != "notifications/initialized" {
        msg["id"] = json!(next_id());
    }
    if let Some(p) = params {
        msg["params"] = p;
    }
    msg
}

async fn mcp_post(
    client: &reqwest::Client,
    url: &str,
    body: &Value,
    auth_token: Option<&str>,
    session_id: Option<&str>,
) -> Result<(Option<String>, Value), String> {
    let mut req = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream");

    if let Some(token) = auth_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    if let Some(sid) = session_id {
        req = req.header("Mcp-Session-Id", sid);
    }

    let resp = req
        .json(body)
        .send()
        .await
        .map_err(|e| format!("MCP request failed: {}", e))?;

    let new_session = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let text = resp.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    if !status.is_success() {
        return Err(format!("MCP HTTP {}: {}", status, text));
    }

    if text.is_empty() || status.as_u16() == 202 || status.as_u16() == 204 {
        return Ok((new_session, Value::Null));
    }

    if content_type.contains("text/event-stream") {
        let parsed = parse_sse_response(&text)?;
        return Ok((new_session, parsed));
    }

    let json_val: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse JSON response: {} — body: {}", e, &text[..text.len().min(200)]))?;

    Ok((new_session, json_val))
}

fn parse_sse_response(text: &str) -> Result<Value, String> {
    let mut last_data = String::new();
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            last_data = data.to_string();
        } else if line.starts_with("data:") {
            last_data = line[5..].trim_start().to_string();
        }
    }
    if last_data.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&last_data)
        .map_err(|e| format!("Failed to parse SSE data: {} — data: {}", e, &last_data[..last_data.len().min(200)]))
}

pub fn read_auth_token(server: &McpServerConfig) -> Option<String> {
    if let Some(ref token) = server.auth_token {
        if !token.is_empty() {
            return Some(token.clone());
        }
    }

    if let Some(ref cmd) = server.auth_command {
        if !cmd.is_empty() {
            return run_auth_command(cmd);
        }
    }

    None
}

fn run_auth_command(cmd: &str) -> Option<String> {
    #[cfg(unix)]
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .ok()?;
    #[cfg(windows)]
    let output = std::process::Command::new("cmd")
        .arg("/C")
        .arg(cmd)
        .output()
        .ok()?;

    if !output.status.success() {
        eprintln!("[mcp] auth command failed: {}", String::from_utf8_lossy(&output.stderr));
        return None;
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

pub fn find_server_config(server_name: &str, server_url: &str) -> Option<McpServerConfig> {
    let servers = load_mcp_config();
    servers.into_iter().find(|s| s.name == server_name || s.server_url == server_url)
}

pub fn load_mcp_config() -> Vec<McpServerConfig> {
    let mut servers = Vec::new();

    if let Some(home) = dirs::home_dir() {
        let config_path = home.join(".config/drover/drover.json");
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                if let Some(obj) = parsed.get("mcpServers").and_then(|v| v.as_object()) {
                    for (name, val) in obj {
                        if let Some(url) = val.get("serverUrl").and_then(|v| v.as_str()) {
                            servers.push(McpServerConfig {
                                name: name.clone(),
                                server_url: url.to_string(),
                                auth_token: val.get("authToken").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                auth_command: val.get("authCommand").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            });
                        }
                    }
                }
            }
        }
    }

    servers
}

#[tauri::command]
pub fn mcp_list_servers() -> Result<Vec<McpServerConfig>, String> {
    Ok(load_mcp_config())
}

#[tauri::command]
pub fn mcp_save_servers(servers: Vec<McpServerConfig>) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let config_dir = home.join(".config/drover");
    let config_path = config_dir.join("drover.json");

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    let mut root = if let Ok(content) = std::fs::read_to_string(&config_path) {
        serde_json::from_str::<Value>(&content).unwrap_or(json!({}))
    } else {
        json!({})
    };

    let mut mcp_obj = serde_json::Map::new();
    for server in &servers {
        let mut entry = serde_json::Map::new();
        entry.insert("serverUrl".to_string(), json!(server.server_url));
        if let Some(ref token) = server.auth_token {
            if !token.is_empty() {
                entry.insert("authToken".to_string(), json!(token));
            }
        }
        if let Some(ref cmd) = server.auth_command {
            if !cmd.is_empty() {
                entry.insert("authCommand".to_string(), json!(cmd));
            }
        }
        mcp_obj.insert(server.name.clone(), Value::Object(entry));
    }

    root.as_object_mut()
        .ok_or("Config root is not an object")?
        .insert("mcpServers".to_string(), Value::Object(mcp_obj));

    let json_str = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    std::fs::write(&config_path, json_str)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn mcp_initialize(server_url: String, server_name: String) -> Result<McpServerInfo, String> {
    let client = build_client()?;
    let config = find_server_config(&server_name, &server_url);
    let auth_token = config.as_ref().and_then(|c| read_auth_token(c));

    let init_body = build_jsonrpc("initialize", Some(json!({
        "protocolVersion": "2025-03-26",
        "capabilities": {},
        "clientInfo": {
            "name": "Drover",
            "version": "0.1.0"
        }
    })));

    let (session_id, init_resp) = mcp_post(
        &client,
        &server_url,
        &init_body,
        auth_token.as_deref(),
        None,
    ).await?;

    let result = init_resp.get("result")
        .ok_or_else(|| {
            if let Some(err) = init_resp.get("error") {
                format!("MCP init error: {}", err)
            } else {
                format!("No result in init response: {}", init_resp)
            }
        })?;

    let notify_body = build_jsonrpc("notifications/initialized", None);
    let _ = mcp_post(
        &client,
        &server_url,
        &notify_body,
        auth_token.as_deref(),
        session_id.as_deref(),
    ).await;

    let server_info_val = result.get("serverInfo").cloned().unwrap_or(json!({}));
    let name = server_info_val.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let version = server_info_val.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
    let protocol_version = result.get("protocolVersion").and_then(|v| v.as_str()).map(|s| s.to_string());
    let capabilities = result.get("capabilities").cloned();

    Ok(McpServerInfo {
        name,
        version,
        protocol_version,
        capabilities,
    })
}

#[tauri::command]
pub async fn mcp_list_tools(server_url: String, server_name: String) -> Result<Vec<McpToolDef>, String> {
    let client = build_client()?;
    let config = find_server_config(&server_name, &server_url);
    let auth_token = config.as_ref().and_then(|c| read_auth_token(c));

    let init_body = build_jsonrpc("initialize", Some(json!({
        "protocolVersion": "2025-03-26",
        "capabilities": {},
        "clientInfo": {
            "name": "Drover",
            "version": "0.1.0"
        }
    })));

    let (session_id, init_resp) = mcp_post(
        &client,
        &server_url,
        &init_body,
        auth_token.as_deref(),
        None,
    ).await?;

    if let Some(err) = init_resp.get("error") {
        return Err(format!("MCP init error: {}", err));
    }

    let notify_body = build_jsonrpc("notifications/initialized", None);
    let _ = mcp_post(
        &client,
        &server_url,
        &notify_body,
        auth_token.as_deref(),
        session_id.as_deref(),
    ).await;

    let list_body = build_jsonrpc("tools/list", None);
    let (_, list_resp) = mcp_post(
        &client,
        &server_url,
        &list_body,
        auth_token.as_deref(),
        session_id.as_deref(),
    ).await?;

    if let Some(err) = list_resp.get("error") {
        return Err(format!("MCP tools/list error: {}", err));
    }

    let tools_val = list_resp
        .get("result")
        .and_then(|r| r.get("tools"))
        .ok_or("No tools in response")?;

    let tools: Vec<McpToolDef> = serde_json::from_value(tools_val.clone())
        .map_err(|e| format!("Failed to parse tools: {}", e))?;

    Ok(tools)
}

#[tauri::command]
pub async fn mcp_call_tool(
    server_url: String,
    server_name: String,
    tool_name: String,
    arguments: Value,
) -> Result<McpToolResult, String> {
    let client = build_client()?;
    let config = find_server_config(&server_name, &server_url);
    let auth_token = config.as_ref().and_then(|c| read_auth_token(c));

    let init_body = build_jsonrpc("initialize", Some(json!({
        "protocolVersion": "2025-03-26",
        "capabilities": {},
        "clientInfo": {
            "name": "Drover",
            "version": "0.1.0"
        }
    })));

    let (session_id, init_resp) = mcp_post(
        &client,
        &server_url,
        &init_body,
        auth_token.as_deref(),
        None,
    ).await?;

    if let Some(err) = init_resp.get("error") {
        return Err(format!("MCP init error: {}", err));
    }

    let notify_body = build_jsonrpc("notifications/initialized", None);
    let _ = mcp_post(
        &client,
        &server_url,
        &notify_body,
        auth_token.as_deref(),
        session_id.as_deref(),
    ).await;

    let call_body = build_jsonrpc("tools/call", Some(json!({
        "name": tool_name,
        "arguments": arguments,
    })));

    let (_, call_resp) = mcp_post(
        &client,
        &server_url,
        &call_body,
        auth_token.as_deref(),
        session_id.as_deref(),
    ).await?;


    if let Some(err) = call_resp.get("error") {
        return Err(format!("MCP tools/call error: {}", err));
    }

    let result_val = call_resp
        .get("result")
        .ok_or("No result in tools/call response")?;

    let tool_result: McpToolResult = serde_json::from_value(result_val.clone())
        .map_err(|e| format!("Failed to parse tool result: {}", e))?;

    Ok(tool_result)
}
