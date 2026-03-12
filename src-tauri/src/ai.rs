use crate::mcp;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::time::Duration;

const AI_API_BASE: &str = "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/";

#[derive(Serialize, Clone)]
pub struct DiffLine {
    pub tag: String,      // "equal", "insert", "delete"
    pub content: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Serialize)]
pub struct CodeEditResult {
    pub file_name: String,
    pub file_path: Option<String>,
    pub original: String,
    pub modified: String,
    pub diff_lines: Vec<DiffLine>,
    pub summary: String,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Multimodal(Vec<ContentPart>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlData },
}

#[derive(Serialize, Deserialize)]
struct ImageUrlData {
    url: String,
}

#[derive(Serialize, Deserialize)]
struct AiMessage {
    role: String,
    content: MessageContent,
}

#[derive(Serialize, Deserialize)]
pub struct AiAttachment {
    pub name: String,
    pub content: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

#[derive(Serialize)]
struct AiRequest {
    messages: Vec<AiMessage>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct VisionRequest {
    messages: Vec<AiMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,  // base64 data URL for vision models
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
- Do NOT wrap commands in bash -c unless necessary.
- ALWAYS single-quote URLs that contain special shell characters (?, &, =, #, etc.). For example: curl 'https://wttr.in/Houston?format=j1'{}{}"#,
        context_block, remote_guidance
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text(system_prompt),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Text(prompt),
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 1024,
    };

    let url = format!("{}@cf/meta/llama-4-scout-17b-16e-instruct", AI_API_BASE.replace("{}", &account_id));

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

    let system_prompt = r#"You are a friendly, knowledgeable assistant in a terminal app called Drover. The user asked a question, commands were executed, and you now have the output. Give a clear, conversational answer.

Rules:
- Answer the user's question directly and naturally, as if talking to them.
- Lead with the key answer or finding — don't bury it.
- Use short paragraphs. Use bullet points only when listing multiple items.
- For structured data (DNS, system info, etc.), present it cleanly with labels.
- If something failed or errored, explain what went wrong and suggest alternatives.
- Keep it concise but complete — don't be terse, be helpful.
- Do NOT use markdown code fences or backticks.
- Do NOT repeat raw command output verbatim.
- Do NOT mention which commands were run unless the user asked about commands."#;

    let user_content = format!(
        "Original question: {}\n\nCommand outputs:\n{}",
        original_prompt, results_block
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text(system_prompt.to_string()),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Text(user_content),
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 2048,
    };

    let url = format!("{}@cf/meta/llama-4-scout-17b-16e-instruct", AI_API_BASE.replace("{}", &account_id));

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
    model: String,
    prompt: String,
    attachments: Vec<AiAttachment>,
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
- ALWAYS single-quote URLs that contain special shell characters (?, &, =, #, etc.). For example: curl 'https://wttr.in/Houston?format=j1'
- If a task is dangerous, respond with: ERROR: <reason>{}{}{}"#,
        context_block, tools_block, &tool_instructions
    );

    // Multi-model pipeline: if images are present, first process with vision model
    let has_images = attachments.iter().any(|a| a.mime_type.starts_with("image/"));
    
    println!("[AI] Attachments count: {}, has_images: {}", attachments.len(), has_images);
    for att in &attachments {
        println!("[AI] Attachment: name={}, mime_type={}, content_len={}", att.name, att.mime_type, att.content.len());
    }
    
    let final_response = if has_images {
        println!("[AI] Processing with vision model...");
        // Step 1: Use vision model to analyze images
        let vision_analysis = process_images_with_vision(
            &account_id,
            &api_token,
            &prompt,
            &attachments,
        ).await?;
        
        // Prefix with ANSWER: so frontend treats it as a direct response, not shell commands
        format!("ANSWER: {}", vision_analysis)
    } else {
        // No images - proceed with normal single-model processing
        process_without_images(
            &account_id,
            &api_token,
            &model,
            &prompt,
            &attachments,
            &context_block,
            &tools_block,
            &tool_instructions,
        ).await?
    };
    
    Ok(final_response)
}

async fn process_images_with_vision(
    account_id: &str,
    api_token: &str,
    prompt: &str,
    attachments: &[AiAttachment],
) -> Result<String, String> {
    // Use Llama 3.2 Vision - it supports the image parameter in REST API
    let vision_model = "@cf/meta/llama-3.2-11b-vision-instruct";
    
    try_vision_request(account_id, api_token, vision_model, prompt, attachments).await
}

async fn try_vision_request(
    account_id: &str,
    api_token: &str,
    vision_model: &str,
    prompt: &str,
    attachments: &[AiAttachment],
) -> Result<String, String> {
    // Find the first image attachment and create data URL
    let image_data_url = attachments
        .iter()
        .find(|a| a.mime_type.starts_with("image/"))
        .map(|a| format!("data:{};base64,{}", a.mime_type, a.content));
    
    // Build multimodal content array with image first, then text
    let mut content_parts = Vec::new();
    
    // Add image to content first
    if let Some(ref data_url) = image_data_url {
        content_parts.push(ContentPart::ImageUrl {
            image_url: ImageUrlData { url: data_url.clone() },
        });
    }
    
    // Then add the user's request
    content_parts.push(ContentPart::Text { 
        text: prompt.to_string()
    });
    
    // Use multimodal message format - image embedded in content array
    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text("You are a vision AI that CAN see and analyze images. An image is attached to this message. NEVER say you cannot see images or that you need text input - you can see the image directly. Analyze what you see and respond to the user's request. Be direct and helpful.".to_string()),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Multimodal(content_parts),
        },
    ];
    
    // Use regular AiRequest - image is embedded in message content, not separate field
    let body = AiRequest {
        messages,
        max_tokens: 2048,
    };
    
    println!("[Vision] Image data URL present: {}, length: {}", image_data_url.is_some(), image_data_url.as_ref().map(|s| s.len()).unwrap_or(0));
    
    let url = format!("{}{}", AI_API_BASE.replace("{}", account_id), vision_model);
    println!("[Vision] URL: {}", url);
    
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
        .map_err(|e| format!("Vision model request failed: {}", e))?;
    
    println!("[Vision] Response status: {}", resp.status());
    
    // Handle license agreement requirement (403 with code 5016)
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        let text = resp.text().await.unwrap_or_default();
        if text.contains("Model Agreement") || text.contains("code\":5016") {
            println!("[Vision] License agreement required, sending 'agree' prompt...");
            
            // Send agreement request using simple prompt format
            let agree_body = serde_json::json!({
                "prompt": "agree"
            });
            
            let agree_resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_token))
                .json(&agree_body)
                .send()
                .await
                .map_err(|e| format!("Agreement request failed: {}", e))?;
            
            let agree_status = agree_resp.status();
            let agree_text = agree_resp.text().await.unwrap_or_default();
            println!("[Vision] Agreement response status: {}, body: {}", agree_status, &agree_text[..agree_text.len().min(200)]);
            
            // Check if agreement was accepted - the API returns 403 but with "Thank you for agreeing" message
            let agreement_accepted = agree_text.contains("Thank you for agreeing") || agree_status.is_success();
            
            if !agreement_accepted {
                return Err(format!("Failed to accept license agreement: {}", agree_text));
            }
            
            println!("[Vision] License agreement accepted!");
            
            // Now retry the original vision request
            println!("[Vision] Agreement accepted, retrying vision request...");
            let retry_resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_token))
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Vision retry request failed: {}", e))?;
            
            println!("[Vision] Retry response status: {}", retry_resp.status());
            
            if !retry_resp.status().is_success() {
                let retry_text = retry_resp.text().await.unwrap_or_default();
                return Err(format!("Vision model error after agreement: {}", retry_text));
            }
            
            let retry_text = retry_resp.text().await.map_err(|e| format!("Failed to read retry response: {}", e))?;
            println!("[Vision] Retry response (first 500 chars): {}", &retry_text[..retry_text.len().min(500)]);
            
            let ai_resp: AiResponse = serde_json::from_str(&retry_text)
                .map_err(|e| format!("Failed to parse vision response: {}", e))?;
            
            if !ai_resp.success {
                return Err(format!("Vision API failed: {:?}", ai_resp.errors));
            }
            
            return ai_resp
                .result
                .and_then(|r| r.response)
                .ok_or_else(|| "No vision response".to_string());
        }
        
        println!("[Vision] Error response: {}", text);
        return Err(format!("Vision model error 403: {}", text));
    }
    
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        println!("[Vision] Error response: {}", text);
        return Err(format!("Vision model error {}: {}", status, text));
    }
    
    let resp_text = resp.text().await.map_err(|e| format!("Failed to read response: {}", e))?;
    println!("[Vision] Response text (first 500 chars): {}", &resp_text[..resp_text.len().min(500)]);
    
    let ai_resp: AiResponse = serde_json::from_str(&resp_text)
        .map_err(|e| format!("Failed to parse vision response: {}", e))?;
    
    if !ai_resp.success {
        return Err(format!("Vision API failed: {:?}", ai_resp.errors));
    }
    
    ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No vision response".to_string())
}

async fn synthesize_with_selected_model(
    account_id: &str,
    api_token: &str,
    model: &str,
    synthesis_prompt: &str,
    context_block: &str,
    tools_block: &str,
    tool_instructions: &str,
) -> Result<String, String> {
    let system_prompt = format!(
        r#"You are an expert terminal assistant in Drover. The user describes tasks in natural language and you respond with executable shell commands OR MCP tool calls.

Rules:
- Respond with ONLY executable shell commands (one per line) or TOOL_CALL directives.
- No explanations, no markdown, no code fences, no commentary.
- ALWAYS single-quote URLs that contain special shell characters (?, &, =, #, etc.). For example: curl 'https://wttr.in/Houston?format=j1'
- If a task is dangerous, respond with: ERROR: <reason>{}{}{}"#,
        context_block, tools_block, tool_instructions
    );
    
    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text(system_prompt),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Text(synthesis_prompt.to_string()),
        },
    ];
    
    let body = AiRequest {
        messages,
        max_tokens: 2048,
    };
    
    let url = format!("{}{}", AI_API_BASE.replace("{}", account_id), model);
    
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
        .map_err(|e| format!("Synthesis request failed: {}", e))?;
    
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Synthesis error {}: {}", status, text));
    }
    
    let ai_resp: AiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse synthesis response: {}", e))?;
    
    if !ai_resp.success {
        return Err(format!("Synthesis API failed: {:?}", ai_resp.errors));
    }
    
    ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No synthesis response".to_string())
}

async fn process_without_images(
    account_id: &str,
    api_token: &str,
    model: &str,
    prompt: &str,
    attachments: &[AiAttachment],
    context_block: &str,
    tools_block: &str,
    tool_instructions: &str,
) -> Result<String, String> {
    // Check if this is an explanation/analysis request (not a task to execute)
    let prompt_lower = prompt.to_lowercase();
    let is_explanation_request = prompt_lower.contains("explain") 
        || prompt_lower.contains("what does") 
        || prompt_lower.contains("how does")
        || prompt_lower.contains("describe")
        || prompt_lower.contains("analyze")
        || prompt_lower.contains("review")
        || prompt_lower.contains("understand");
    
    let has_text_attachments = attachments.iter().any(|a| !a.mime_type.starts_with("image/") && a.mime_type != "application/pdf");
    
    // If asking for explanation with text file attachments, use explanation mode
    if is_explanation_request && has_text_attachments {
        return process_explanation_request(account_id, api_token, model, prompt, attachments).await;
    }
    
    let system_prompt = format!(
        r#"You are an expert terminal assistant in Drover. The user describes tasks in natural language and you respond with executable shell commands OR MCP tool calls.

Rules:
- Respond with ONLY executable shell commands (one per line) or TOOL_CALL directives.
- No explanations, no markdown, no code fences, no commentary.
- ALWAYS single-quote URLs that contain special shell characters (?, &, =, #, etc.). For example: curl 'https://wttr.in/Houston?format=j1'
- If a task is dangerous, respond with: ERROR: <reason>{}{}{}"#,
        context_block, tools_block, tool_instructions
    );
    
    // Build user message with text attachments only (no images in this path)
    let mut user_content = prompt.to_string();
    if !attachments.is_empty() {
        user_content.push_str("\n\nAttached files:\n");
        for attachment in attachments.iter() {
            user_content.push_str(&format!("\n--- {} ---\n", attachment.name));
            if attachment.mime_type == "application/pdf" {
                user_content.push_str("[PDF file - not supported]\n");
            } else if !attachment.mime_type.starts_with("image/") {
                user_content.push_str(&attachment.content);
                user_content.push('\n');
            }
        }
    }
    
    let user_message = AiMessage {
        role: "user".to_string(),
        content: MessageContent::Text(user_content),
    };

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text(system_prompt),
        },
        user_message,
    ];

    let body = AiRequest {
        messages,
        max_tokens: 2048,
    };

    let url = format!("{}{}", AI_API_BASE.replace("{}", account_id), model);

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

async fn process_explanation_request(
    account_id: &str,
    api_token: &str,
    model: &str,
    prompt: &str,
    attachments: &[AiAttachment],
) -> Result<String, String> {
    println!("[AI] Processing explanation request for text file attachments...");
    
    // Build content with attached files
    let mut user_content = prompt.to_string();
    user_content.push_str("\n\nAttached files:\n");
    for attachment in attachments.iter() {
        if !attachment.mime_type.starts_with("image/") && attachment.mime_type != "application/pdf" {
            user_content.push_str(&format!("\n--- {} ---\n", attachment.name));
            user_content.push_str(&attachment.content);
            user_content.push('\n');
        }
    }
    
    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text("You are a helpful coding assistant. Explain code clearly and thoroughly. Describe what the code does, how it works, and any important patterns or concepts used. Be direct and informative.".to_string()),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Text(user_content),
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 2048,
    };

    let url = format!("{}{}", AI_API_BASE.replace("{}", account_id), model);

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

    let explanation = ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No AI response".to_string())?;
    
    // Return with ANSWER: prefix so frontend displays it directly
    Ok(format!("ANSWER: {}", explanation))
}

#[derive(Deserialize)]
pub struct FailedAttempt {
    pub command: String,
    pub error: String,
}

#[tauri::command]
pub async fn ai_retry(
    account_id: String,
    api_token: String,
    original_prompt: String,
    failed_attempts: Vec<FailedAttempt>,
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

    let mut attempts_block = String::new();
    for (i, attempt) in failed_attempts.iter().enumerate() {
        attempts_block.push_str(&format!(
            "Attempt {}:\n  Command: {}\n  Error: {}\n\n",
            i + 1,
            attempt.command,
            attempt.error
        ));
    }

    let system_prompt = format!(
        r#"You are an expert terminal assistant in Drover. A previous command failed. Analyze the error and respond with an alternative command that accomplishes the same goal.

Rules:
- Respond with ONLY executable shell commands (one per line), OR a single line starting with ANSWER: if the task needs no commands (e.g. math, general knowledge).
- If the error indicates the command/tool is not available, use an alternative tool or approach that achieves the same result.
- If the error is a permissions issue, suggest the appropriate fix (e.g. Run As Administrator, sudo).
- If the task is simply impossible in this environment, respond with: ERROR: <clear explanation of why and what the user can do instead>
- ALWAYS single-quote URLs that contain special shell characters (?, &, =, #, etc.).
- No explanations, no markdown, no code fences.
- Do NOT retry the exact same command that already failed.{}"#,
        context_block
    );

    let user_content = format!(
        "Original request: {}\n\nFailed attempts:\n{}Provide an alternative approach.",
        original_prompt, attempts_block
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text(system_prompt),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Text(user_content),
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 2048,
    };

    let url = format!("{}@cf/meta/llama-4-scout-17b-16e-instruct", AI_API_BASE.replace("{}", &account_id));

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
        .map_err(|e| format!("AI retry request failed: {}", e))?;

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

    ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No AI response".to_string())
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

    let system_prompt = r#"You are a friendly, knowledgeable assistant in a terminal app called Drover. The user asked a question, and tools/commands were executed to get the answer. Give a clear, conversational answer.

Rules:
- Answer the user's question directly and naturally, as if talking to them.
- Lead with the key answer or finding — don't bury it.
- Use short paragraphs. Use bullet points only when listing multiple items.
- For structured data, present it cleanly with labels.
- If something failed or errored, explain what went wrong and suggest alternatives.
- Keep it concise but complete — don't be terse, be helpful.
- Do NOT use markdown code fences or backticks.
- Do NOT repeat raw JSON or command output verbatim — extract key information.
- Do NOT mention which commands were run unless the user asked about commands."#;

    let user_content = format!(
        "Original question: {}\n\nResults:\n{}",
        original_prompt, results_block
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text(system_prompt.to_string()),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Text(user_content),
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 4096,
    };

    let url = format!("{}@cf/meta/llama-4-scout-17b-16e-instruct", AI_API_BASE.replace("{}", &account_id));

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

#[tauri::command]
pub async fn ai_edit_code(
    account_id: String,
    api_token: String,
    model: String,
    file_name: String,
    file_path: Option<String>,
    file_content: String,
    instruction: String,
) -> Result<CodeEditResult, String> {
    println!("[AI Edit] Processing code edit request for: {}", file_name);
    println!("[AI Edit] Instruction: {}", instruction);
    
    let system_prompt = r#"You are an expert code editor. The user will provide a code file and an instruction for how to modify it.

Your task:
1. Analyze the existing code
2. Make the requested modifications
3. Return ONLY the complete modified file content

Rules:
- Return the ENTIRE modified file, not just the changed parts
- Preserve the original code style, indentation, and formatting
- Do not add explanatory comments unless specifically requested
- Do not wrap the code in markdown code fences
- Output ONLY the modified code, nothing else"#;

    let user_content = format!(
        "File: {}\n\nOriginal code:\n{}\n\nInstruction: {}",
        file_name, file_content, instruction
    );

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text(system_prompt.to_string()),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Text(user_content),
        },
    ];

    let body = AiRequest {
        messages,
        max_tokens: 4096,
    };

    let url = format!("{}{}", AI_API_BASE.replace("{}", &account_id), model);

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
        .map_err(|e| format!("Code edit request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Code edit error {}: {}", status, text));
    }

    let ai_resp: AiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse code edit response: {}", e))?;

    if !ai_resp.success {
        return Err(format!("Code edit failed: {:?}", ai_resp.errors));
    }

    let modified = ai_resp
        .result
        .and_then(|r| r.response)
        .ok_or_else(|| "No code edit response".to_string())?;

    // Generate diff
    let diff_lines = generate_diff(&file_content, &modified);
    
    // Generate summary
    let summary = generate_edit_summary(&file_content, &modified, &instruction);

    println!("[AI Edit] Generated {} diff lines", diff_lines.len());
    println!("[AI Edit] Summary: {}", summary);

    Ok(CodeEditResult {
        file_name,
        file_path,
        original: file_content,
        modified,
        diff_lines,
        summary,
    })
}

fn generate_diff(original: &str, modified: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(original, modified);
    let mut diff_lines = Vec::new();
    let mut old_line = 1usize;
    let mut new_line = 1usize;

    for change in diff.iter_all_changes() {
        let (tag, old_ln, new_ln) = match change.tag() {
            ChangeTag::Equal => {
                let result = ("equal", Some(old_line), Some(new_line));
                old_line += 1;
                new_line += 1;
                result
            }
            ChangeTag::Delete => {
                let result = ("delete", Some(old_line), None);
                old_line += 1;
                result
            }
            ChangeTag::Insert => {
                let result = ("insert", None, Some(new_line));
                new_line += 1;
                result
            }
        };

        diff_lines.push(DiffLine {
            tag: tag.to_string(),
            content: change.value().to_string(),
            old_line: old_ln,
            new_line: new_ln,
        });
    }

    diff_lines
}

#[tauri::command]
pub async fn ai_edit_code_from_path(
    account_id: String,
    api_token: String,
    model: String,
    file_path: String,
    instruction: String,
    cwd: Option<String>,
) -> Result<CodeEditResult, String> {
    use std::path::PathBuf;
    
    let home = std::env::var("HOME").unwrap_or_default();
    
    // Helper to expand ~ in paths
    let expand_tilde = |p: &str| -> String {
        if p.starts_with('~') {
            p.replacen('~', &home, 1)
        } else {
            p.to_string()
        }
    };
    
    // Resolve the file path (handle relative paths)
    let resolved_path = if file_path.starts_with('/') {
        file_path.clone()
    } else if file_path.starts_with('~') {
        expand_tilde(&file_path)
    } else if let Some(ref cwd) = cwd {
        // Relative path - resolve against cwd (also expand ~ in cwd)
        let expanded_cwd = expand_tilde(cwd);
        let mut path = PathBuf::from(&expanded_cwd);
        path.push(&file_path);
        path.to_string_lossy().to_string()
    } else {
        file_path.clone()
    };
    
    println!("[AI Edit from Path] cwd: {:?}, file_path: {}, resolved: {}", cwd, file_path, resolved_path);
    
    // Read the file content
    let file_content = std::fs::read_to_string(&resolved_path)
        .map_err(|e| format!("Failed to read file '{}': {}", resolved_path, e))?;
    
    // Extract file name from path
    let file_name = PathBuf::from(&resolved_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.clone());
    
    // Delegate to the existing ai_edit_code function
    ai_edit_code(
        account_id,
        api_token,
        model,
        file_name,
        Some(resolved_path),
        file_content,
        instruction,
    ).await
}

fn generate_edit_summary(original: &str, modified: &str, instruction: &str) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();
    
    let diff = TextDiff::from_lines(original, modified);
    let mut insertions = 0;
    let mut deletions = 0;
    
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => insertions += 1,
            ChangeTag::Delete => deletions += 1,
            _ => {}
        }
    }

    let line_diff = mod_lines.len() as i32 - orig_lines.len() as i32;
    let line_change = if line_diff > 0 {
        format!("+{} lines", line_diff)
    } else if line_diff < 0 {
        format!("{} lines", line_diff)
    } else {
        "no net change in lines".to_string()
    };

    format!(
        "Modified file based on: \"{}\"\n{} insertions, {} deletions ({})",
        instruction, insertions, deletions, line_change
    )
}
