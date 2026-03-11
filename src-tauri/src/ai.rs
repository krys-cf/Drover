use crate::mcp;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const AI_MODEL_URL: &str = "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/@cf/meta/llama-4-scout-17b-16e-instruct";

#[derive(Serialize, Deserialize)]
struct AiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct AiRequest {
    messages: Vec<AiMessage>,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct AiResultInner {
    response: Option<String>,
}

#[derive(Deserialize)]
struct AiResponse {
    result: Option<AiResultInner>,
    success: bool,
    errors: Vec<serde_json::Value>,
}

#[tauri::command]
pub async fn ai_chat(
    account_id: String,
    api_token: String,
    prompt: String,
    cwd: Option<String>,
    os_info: Option<String>,
    shell: Option<String>,
    ssh_target: Option<String>,
) -> Result<String, String> {
    let mut context_lines = Vec::new();
    if let Some(ref c) = cwd {
        context_lines.push(format!("Current directory: {}", c));
    }
    if let Some(ref o) = os_info {
        context_lines.push(format!("OS: {}", o));
    }
    if let Some(ref s) = shell {
        context_lines.push(format!("Shell: {}", s));
    }
    if let Some(ref t) = ssh_target {
        context_lines.push(format!("Connected via SSH to: {}", t));
    }

    let context_block = if context_lines.is_empty() {
        String::new()
    } else {
        format!("\n\nCurrent environment:\n{}", context_lines.join("\n"))
    };

    let remote_guidance = if ssh_target.is_some() {
        r#"

Remote server guidance:
- Commands will execute on the REMOTE server, not the local machine.
- Use Linux-compatible commands (systemctl, journalctl, apt/yum, etc.) as appropriate for the OS.
- For service inspection: prefer systemctl status, journalctl -u, ss -tlnp over legacy alternatives.
- For disk/memory: use df -h, free -h, top -bn1 | head, etc.
- For logs: use journalctl, tail -f /var/log/*, or dmesg as appropriate.
- Do NOT use macOS-specific commands (brew, launchctl, pbcopy, open, etc.) on Linux servers.
- If the user asks about the server itself (uptime, load, disk, services), answer about the remote server."#
    } else {
        ""
    };

    let system_prompt = format!(
        r#"You are an expert systems administrator and terminal assistant built into a terminal app called Drover. The user describes tasks in natural language and you respond with the exact shell commands to execute.

Rules:
- Respond with ONLY executable shell commands, one per line.
- No explanations, no markdown, no code fences, no commentary, no numbering.
- Each line must be a complete, standalone command that can be piped to a shell.
- For multi-step tasks, output each command on its own line in execution order.
- Use absolute paths when the task references specific directories/files.
- For file creation with content, use heredoc syntax (cat << 'EOF' > file ... EOF) or echo/printf.
- If a task is ambiguous, make reasonable assumptions and proceed.
- If a task is genuinely dangerous (rm -rf /, drop database in production), respond with a single line: ERROR: <reason>
- Prefer simple, portable commands (POSIX where possible).
- Do NOT wrap commands in bash -c unless necessary.{}{}"#,
        context_block, remote_guidance
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        AiMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 1024,
    };

    let url = AI_MODEL_URL.replace("{}", &account_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_token))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }

    let ai_resp: AiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    if !ai_resp.success {
        return Err(format!("AI API error: {:?}", ai_resp.errors));
    }

    ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No response from AI".to_string())
}

#[derive(Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub output: String,
}

#[tauri::command]
pub async fn ai_summarize(
    account_id: String,
    api_token: String,
    original_prompt: String,
    command_results: Vec<CommandResult>,
) -> Result<String, String> {
    let mut results_block = String::new();
    for cr in &command_results {
        results_block.push_str(&format!("$ {}\n{}\n\n", cr.command, cr.output));
    }

    let system_prompt = r#"You are a helpful terminal assistant in an app called Drover. The user asked a question, commands were executed, and you now have the output. Summarize the results in a clear, concise, human-readable way.

Rules:
- Be concise and direct. Use short paragraphs or bullet points.
- Highlight the most important findings first.
- If there are DNS records, format them cleanly (type, value, TTL).
- If there are errors in the output, mention them clearly.
- Do NOT repeat the raw command output verbatim — summarize it.
- Do NOT use markdown code fences. Use plain text.
- Mention which commands were run, briefly, at the end if relevant."#;

    let user_content = format!(
        "Original question: {}\n\nCommand outputs:\n{}",
        original_prompt, results_block
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        AiMessage {
            role: "user".to_string(),
            content: user_content,
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 2048,
    };

    let url = AI_MODEL_URL.replace("{}", &account_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_token))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }

    let ai_resp: AiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    if !ai_resp.success {
        return Err(format!("AI API error: {:?}", ai_resp.errors));
    }

    ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No response from AI".to_string())
}

fn build_tools_prompt(tools: &[(String, String, Vec<mcp::McpToolDef>)]) -> String {
    let mut prompt = String::from("Available MCP tools:\n\n");
    for (server_name, _server_url, tool_list) in tools {
        prompt.push_str(&format!("[{}]\n", server_name));
        for tool in tool_list {
            prompt.push_str(&format!("  - {}", tool.name));
            if let Some(ref desc) = tool.description {
                prompt.push_str(&format!(": {}", desc));
            }
            prompt.push('\n');
            if let Some(ref schema) = tool.input_schema {
                if let Some(props) = schema.get("properties") {
                    let required: Vec<String> = schema.get("required")
                        .and_then(|r| r.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    if let Some(obj) = props.as_object() {
                        for (pname, pval) in obj {
                            let ptype = pval.get("type").and_then(|v| v.as_str()).unwrap_or("any");
                            let pdesc = pval.get("description").and_then(|v| v.as_str()).unwrap_or("");
                            let req = if required.contains(pname) { " (required)" } else { "" };
                            prompt.push_str(&format!("      {}: {}{}\n", pname, ptype, req));
                            if !pdesc.is_empty() {
                                prompt.push_str(&format!("        {}\n", pdesc));
                            }
                        }
                    }
                }
            }
        }
        prompt.push('\n');
    }
    prompt
}

fn build_tool_example(tools: &[(String, String, Vec<mcp::McpToolDef>)]) -> String {
    for (server_name, _server_url, tool_list) in tools {
        if let Some(tool) = tool_list.first() {
            let example_args = if let Some(ref schema) = tool.input_schema {
                if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                    let pairs: Vec<String> = props.keys().take(2).map(|k| format!("\"{}\": \"...\"", k)).collect();
                    if pairs.is_empty() { "{}".to_string() } else { format!("{{{}}}", pairs.join(", ")) }
                } else {
                    "{}".to_string()
                }
            } else {
                "{}".to_string()
            };
            return format!(
                "\nExample:\nTOOL_CALL: {} | {} | {}\n",
                server_name, tool.name, example_args
            );
        }
    }
    String::new()
}

#[derive(Serialize, Deserialize)]
pub struct ToolCallResult {
    pub server_name: String,
    pub tool_name: String,
    pub arguments: String,
    pub output: String,
    pub is_error: bool,
}

#[tauri::command]
pub async fn ai_chat_with_tools(
    account_id: String,
    api_token: String,
    prompt: String,
    cwd: Option<String>,
    os_info: Option<String>,
    shell: Option<String>,
    ssh_target: Option<String>,
) -> Result<String, String> {
    let servers = mcp::load_mcp_config();

    let mut all_tools: Vec<(String, String, Vec<mcp::McpToolDef>)> = Vec::new();
    for server in &servers {
        if mcp::read_auth_token(server).is_none() {
            continue;
        }
        match mcp::mcp_list_tools(server.server_url.clone(), server.name.clone()).await {
            Ok(tools) => {
                all_tools.push((server.name.clone(), server.server_url.clone(), tools));
            }
            Err(e) => {
                eprintln!("Failed to list tools from {}: {}", server.name, e);
            }
        }
    }

    let has_tools = all_tools.iter().any(|(_, _, t)| !t.is_empty());

    let mut context_lines = Vec::new();
    if let Some(ref c) = cwd {
        context_lines.push(format!("Current directory: {}", c));
    }
    if let Some(ref o) = os_info {
        context_lines.push(format!("OS: {}", o));
    }
    if let Some(ref s) = shell {
        context_lines.push(format!("Shell: {}", s));
    }
    if let Some(ref t) = ssh_target {
        context_lines.push(format!("Connected via SSH to: {}", t));
    }

    let context_block = if context_lines.is_empty() {
        String::new()
    } else {
        format!("\n\nCurrent environment:\n{}", context_lines.join("\n"))
    };

    let tools_block = if has_tools {
        format!("\n\n{}", build_tools_prompt(&all_tools))
    } else {
        String::new()
    };

    let tool_instructions = if has_tools {
        let example = build_tool_example(&all_tools);
        format!(r#"

To use an MCP tool, you MUST output a line starting with "TOOL_CALL:" in this exact format:
TOOL_CALL: server_name | tool_name | {{"param": "value"}}
{}
CRITICAL RULES:
- server_name MUST be the exact name in [brackets] above. It is NOT derived from the tool name prefix.
- tool_name is the full tool name exactly as listed.
- Arguments must be valid JSON.
- Every MCP tool invocation MUST use TOOL_CALL: format. Never output a tool name as a bare shell command.
- Do NOT explain tool calls — just output them."#, example)
    } else {
        String::new()
    };

    let system_prompt = format!(
        r#"You are an expert terminal assistant in Drover. The user describes tasks in natural language and you respond with executable shell commands OR MCP tool calls.

Rules:
- Respond with ONLY executable shell commands (one per line) or TOOL_CALL directives.
- No explanations, no markdown, no code fences, no commentary.
- If a task is dangerous, respond with: ERROR: <reason>{}{}{}"#,
        context_block, tools_block, &tool_instructions
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        AiMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 2048,
    };

    let url = AI_MODEL_URL.replace("{}", &account_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_token))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("AI request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("AI error {}: {}", status, text));
    }

    let ai_resp: AiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse AI response: {}", e))?;

    if !ai_resp.success {
        return Err(format!("AI API failed: {:?}", ai_resp.errors));
    }

    let result = ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No AI response".to_string())?;
    Ok(result)
}

#[tauri::command]
pub async fn ai_summarize_tool_results(
    account_id: String,
    api_token: String,
    original_prompt: String,
    tool_results: Vec<ToolCallResult>,
    command_results: Vec<CommandResult>,
) -> Result<String, String> {
    let mut results_block = String::new();

    for tr in &tool_results {
        results_block.push_str(&format!(
            "[MCP Tool: {} on {}]\nArguments: {}\n{}{}\n\n",
            tr.tool_name,
            tr.server_name,
            tr.arguments,
            if tr.is_error { "ERROR: " } else { "" },
            tr.output
        ));
    }

    for cr in &command_results {
        results_block.push_str(&format!("$ {}\n{}\n\n", cr.command, cr.output));
    }

    let system_prompt = r#"You are a helpful terminal assistant in an app called Drover. The user asked a question, and tools/commands were executed to get the answer. Summarize the results clearly and concisely.

Rules:
- Be concise and direct. Use short paragraphs or bullet points.
- Highlight the most important findings first.
- Format structured data clearly with relevant details.
- If there are errors, mention them clearly.
- Do NOT repeat raw JSON output verbatim — extract and present the key information.
- Do NOT use markdown code fences. Use plain text.
- Format lists cleanly with dashes or bullets."#;

    let user_content = format!(
        "Original question: {}\n\nResults:\n{}",
        original_prompt, results_block
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        AiMessage {
            role: "user".to_string(),
            content: user_content,
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 4096,
    };

    let url = AI_MODEL_URL.replace("{}", &account_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_token))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Summarize request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Summarize error {}: {}", status, text));
    }

    let ai_resp: AiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse summarize response: {}", e))?;

    if !ai_resp.success {
        return Err(format!("Summarize failed: {:?}", ai_resp.errors));
    }

    ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No summarize response".to_string())
}
