use crate::mcp;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::time::Duration;

const DEFAULT_KURATCHI_BASE_URL: &str = "https://kuratchi.cloud";
const DEFAULT_KURATCHI_MODEL: &str = "@cf/meta/llama-4-scout-17b-16e-instruct";
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

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
struct KuratchiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KuratchiChatRequest {
    session_id: Option<String>,
    model: Option<String>,
    messages: Vec<KuratchiMessage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KuratchiChatData {
    session_id: String,
    reply: String,
}

#[derive(Deserialize)]
struct KuratchiChatEnvelope {
    success: bool,
    data: Option<KuratchiChatData>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct KuratchiSessionSummary {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub model: String,
    #[serde(default, alias = "lastUserMessage")]
    pub last_user_message: Option<String>,
    #[serde(default, alias = "lastAssistantMessage")]
    pub last_assistant_message: Option<String>,
    #[serde(default, alias = "messageCount")]
    pub message_count: i32,
    #[serde(alias = "createdAt")]
    pub created_at: String,
    #[serde(alias = "updatedAt")]
    pub updated_at: String,
}

#[derive(Deserialize)]
struct KuratchiSessionsEnvelope {
    success: bool,
    data: Option<Vec<KuratchiSessionSummary>>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateSessionRequest {
    title: Option<String>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct CreateSessionEnvelope {
    success: bool,
    data: Option<KuratchiSessionSummary>,
    error: Option<String>,
}

#[derive(Serialize)]
pub struct AiChatTurn {
    pub session_id: String,
    pub response: String,
}

fn kuratchi_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        DEFAULT_KURATCHI_BASE_URL.to_string()
    } else {
        trimmed.trim_end_matches('/').to_string()
    }
}

fn build_kuratchi_url(base_url: &str, path: &str) -> String {
    format!("{}/api/v1{}", kuratchi_base_url(base_url), path)
}

fn build_http_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

fn message_to_kuratchi(message: AiMessage) -> Result<KuratchiMessage, String> {
    match message.content {
        MessageContent::Text(content) => Ok(KuratchiMessage {
            role: message.role,
            content,
        }),
        MessageContent::Multimodal(_) => Err("Image attachments are not supported with Kuratchi AI sessions yet".to_string()),
    }
}

async fn kuratchi_chat(
    base_url: &str,
    api_token: &str,
    session_id: Option<String>,
    model: Option<String>,
    messages: Vec<AiMessage>,
    timeout_secs: u64,
) -> Result<KuratchiChatData, String> {
    let body = KuratchiChatRequest {
        session_id,
        model,
        messages: messages
            .into_iter()
            .map(message_to_kuratchi)
            .collect::<Result<Vec<_>, _>>()?,
    };

    let client = build_http_client(timeout_secs)?;
    let resp = client
        .post(build_kuratchi_url(base_url, "/ai/chat"))
        .header("Authorization", format!("Bearer {}", api_token))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Kuratchi request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Kuratchi API error {}: {}", status, text));
    }

    let parsed: KuratchiChatEnvelope = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Kuratchi response: {}", e))?;

    if !parsed.success {
        return Err(parsed.error.unwrap_or_else(|| "Kuratchi AI request failed".to_string()));
    }

    parsed
        .data
        .ok_or_else(|| "Kuratchi AI response was missing data".to_string())
}

async fn kuratchi_create_session(
    base_url: &str,
    api_token: &str,
    title: Option<String>,
    model: Option<String>,
) -> Result<KuratchiSessionSummary, String> {
    let client = build_http_client(30)?;
    let resp = client
        .post(build_kuratchi_url(base_url, "/ai/sessions"))
        .header("Authorization", format!("Bearer {}", api_token))
        .json(&CreateSessionRequest { title, model })
        .send()
        .await
        .map_err(|e| format!("Create session request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Create session error {}: {}", status, text));
    }

    let parsed: CreateSessionEnvelope = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse create session response: {}", e))?;

    if !parsed.success {
        return Err(parsed.error.unwrap_or_else(|| "Create session failed".to_string()));
    }

    parsed
        .data
        .ok_or_else(|| "Create session response was missing data".to_string())
}

// ── OpenAI-compatible API (covers OpenAI, Groq, OpenRouter, Ollama, etc.) ────

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: OpenAiContent,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum OpenAiContent {
    Text(String),
    Parts(Vec<OpenAiContentPart>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum OpenAiContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: OpenAiImageUrl },
}

#[derive(Serialize, Deserialize)]
struct OpenAiImageUrl {
    url: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Option<Vec<OpenAiChoice>>,
    error: Option<OpenAiError>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiError {
    message: String,
}

async fn openai_compatible_chat(
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<OpenAiMessage>,
    timeout_secs: u64,
) -> Result<String, String> {
    let url = format!(
        "{}/chat/completions",
        base_url.trim_end_matches('/')
    );

    let body = OpenAiRequest {
        model: model.to_string(),
        messages,
        max_tokens: Some(4096),
    };

    let client = build_http_client(timeout_secs)?;
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body);

    // Only add auth header if api_key is non-empty (Ollama/LM Studio don't need one)
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("OpenAI-compatible request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }

    let parsed: OpenAiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    if let Some(error) = parsed.error {
        return Err(format!("API error: {}", error.message));
    }

    parsed
        .choices
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.message.content)
        .ok_or_else(|| "API returned no content".to_string())
}

#[tauri::command]
pub async fn ai_openai_chat(
    base_url: String,
    api_key: String,
    model: String,
    prompt: String,
    attachments: Vec<AiAttachment>,
    cwd: Option<String>,
    os_info: Option<String>,
    shell: Option<String>,
    ssh_target: Option<String>,
) -> Result<String, String> {
    // Load MCP tools
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

To use an MCP tool, output a line starting with "TOOL_CALL:" in this exact format:
TOOL_CALL: server_name | tool_name | {{"param": "value"}}
{}
CRITICAL RULES:
- server_name MUST be the exact name in [brackets] above.
- tool_name is the full tool name exactly as listed.
- Arguments must be valid JSON.
- Every MCP tool invocation MUST use TOOL_CALL: format.
- Do NOT explain tool calls — just output them."#, example)
    } else {
        String::new()
    };

    // Check if explanation request
    let prompt_lower = prompt.to_lowercase();
    let is_explanation = prompt_lower.contains("explain")
        || prompt_lower.contains("what does")
        || prompt_lower.contains("how does")
        || prompt_lower.contains("describe")
        || prompt_lower.contains("analyze")
        || prompt_lower.contains("review");

    let has_text_attachments = attachments
        .iter()
        .any(|a| !a.mime_type.starts_with("image/") && a.mime_type != "application/pdf");

    let system_prompt = if is_explanation && has_text_attachments {
        "You are a helpful coding assistant. Explain code clearly and thoroughly. Describe what the code does, how it works, and any important patterns or concepts used. Be direct and informative.".to_string()
    } else {
        format!(
            r#"You are an expert terminal assistant in Drover. The user describes tasks in natural language and you respond with executable shell commands OR MCP tool calls.

Rules:
- Respond with ONLY executable shell commands (one per line) or TOOL_CALL directives.
- No explanations, no markdown, no code fences, no commentary.
- ALWAYS single-quote URLs that contain special shell characters (?, &, =, #, etc.).
- If a task is dangerous, respond with: ERROR: <reason>
- If the question is conversational or doesn't need commands, respond with: ANSWER: <your response>{}{}{}"#,
            context_block, tools_block, tool_instructions
        )
    };

    // Build messages
    let mut messages = vec![OpenAiMessage {
        role: "system".to_string(),
        content: OpenAiContent::Text(system_prompt),
    }];

    // Build user message — check for image attachments
    let has_images = attachments.iter().any(|a| a.mime_type.starts_with("image/"));

    if has_images {
        let mut parts: Vec<OpenAiContentPart> = Vec::new();

        let mut user_text = prompt.clone();
        for attachment in &attachments {
            if !attachment.mime_type.starts_with("image/") && attachment.mime_type != "application/pdf" {
                user_text.push_str(&format!("\n\n--- {} ---\n{}", attachment.name, attachment.content));
            }
        }
        parts.push(OpenAiContentPart::Text { text: user_text });

        for attachment in &attachments {
            if attachment.mime_type.starts_with("image/") {
                parts.push(OpenAiContentPart::ImageUrl {
                    image_url: OpenAiImageUrl {
                        url: format!("data:{};base64,{}", attachment.mime_type, attachment.content),
                    },
                });
            }
        }

        messages.push(OpenAiMessage {
            role: "user".to_string(),
            content: OpenAiContent::Parts(parts),
        });
    } else {
        let mut user_text = prompt.clone();
        for attachment in &attachments {
            if !attachment.mime_type.starts_with("image/") && attachment.mime_type != "application/pdf" {
                user_text.push_str(&format!("\n\n--- {} ---\n{}", attachment.name, attachment.content));
            }
        }
        messages.push(OpenAiMessage {
            role: "user".to_string(),
            content: OpenAiContent::Text(user_text),
        });
    }

    let reply = openai_compatible_chat(&base_url, &api_key, &model, messages, 60).await?;

    if is_explanation && has_text_attachments && !reply.starts_with("ANSWER:") {
        Ok(format!("ANSWER: {}", reply))
    } else {
        Ok(reply)
    }
}

#[tauri::command]
pub async fn ai_openai_edit_code(
    base_url: String,
    api_key: String,
    model: String,
    file_name: String,
    file_path: Option<String>,
    file_content: String,
    instruction: String,
) -> Result<CodeEditResult, String> {
    let messages = vec![
        OpenAiMessage {
            role: "system".to_string(),
            content: OpenAiContent::Text(r#"You are an expert code editor. The user will provide a code file and an instruction for how to modify it.

Your task:
1. Analyze the existing code
2. Make the requested modifications
3. Return ONLY the complete modified file content

Rules:
- Return the ENTIRE modified file, not just the changed parts
- Preserve the original code style, indentation, and formatting
- Do not add explanatory comments unless specifically requested
- Do not wrap the code in markdown code fences
- Output ONLY the modified code, nothing else"#.to_string()),
        },
        OpenAiMessage {
            role: "user".to_string(),
            content: OpenAiContent::Text(format!(
                "File: {}\n\nOriginal code:\n{}\n\nInstruction: {}",
                file_name, file_content, instruction
            )),
        },
    ];

    let modified = openai_compatible_chat(&base_url, &api_key, &model, messages, 60).await?;

    let diff_lines = generate_diff(&file_content, &modified);
    let summary = generate_edit_summary(&file_content, &modified, &instruction);

    Ok(CodeEditResult {
        file_name,
        file_path,
        original: file_content,
        modified,
        diff_lines,
        summary,
    })
}

#[tauri::command]
pub async fn ai_openai_edit_code_from_path(
    base_url: String,
    api_key: String,
    model: String,
    file_path: String,
    instruction: String,
    cwd: Option<String>,
) -> Result<CodeEditResult, String> {
    use std::path::PathBuf;

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    let expand_tilde = |p: &str| -> String {
        if p.starts_with('~') { p.replacen('~', &home, 1) } else { p.to_string() }
    };

    let resolved_path = if file_path.starts_with('/') || file_path.contains(':') {
        file_path.clone()
    } else if file_path.starts_with('~') {
        expand_tilde(&file_path)
    } else if let Some(ref cwd) = cwd {
        let mut path = PathBuf::from(expand_tilde(cwd));
        path.push(&file_path);
        path.to_string_lossy().to_string()
    } else {
        file_path.clone()
    };

    let file_content = std::fs::read_to_string(&resolved_path)
        .map_err(|e| format!("Failed to read file '{}': {}", resolved_path, e))?;

    let file_name = PathBuf::from(&resolved_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.clone());

    ai_openai_edit_code(base_url, api_key, model, file_name, Some(resolved_path), file_content, instruction).await
}

// ── Gemini API types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    response_modalities: Option<Vec<String>>,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
    InlineData { inline_data: GeminiInlineData },
}

#[derive(Serialize, Deserialize)]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    error: Option<GeminiError>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiCandidateContent,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Vec<GeminiResponsePart>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
    inline_data: Option<GeminiResponseInlineData>,
}

#[derive(Deserialize)]
struct GeminiResponseInlineData {
    mime_type: String,
    data: String,
}

#[derive(Deserialize)]
struct GeminiError {
    message: String,
}

async fn gemini_chat(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    contents: Vec<GeminiContent>,
    timeout_secs: u64,
) -> Result<String, String> {
    let body = GeminiRequest {
        contents,
        system_instruction: if system_prompt.is_empty() {
            None
        } else {
            Some(GeminiSystemInstruction {
                parts: vec![GeminiPart::Text {
                    text: system_prompt.to_string(),
                }],
            })
        },
        generation_config: None,
    };

    let url = format!(
        "{}/models/{}:generateContent?key={}",
        GEMINI_API_BASE, model, api_key
    );

    let client = build_http_client(timeout_secs)?;
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini API error {}: {}", status, text));
    }

    let parsed: GeminiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

    if let Some(error) = parsed.error {
        return Err(format!("Gemini error: {}", error.message));
    }

    let text = parsed
        .candidates
        .and_then(|c| c.into_iter().next())
        .and_then(|c| {
            c.content
                .parts
                .into_iter()
                .filter_map(|p| p.text)
                .next()
        })
        .ok_or_else(|| "Gemini returned no content".to_string())?;

    Ok(text)
}

#[tauri::command]
pub async fn ai_gemini_chat(
    api_key: String,
    model: String,
    prompt: String,
    attachments: Vec<AiAttachment>,
    cwd: Option<String>,
    os_info: Option<String>,
    shell: Option<String>,
    ssh_target: Option<String>,
) -> Result<String, String> {
    // Load MCP tools
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
- server_name MUST be the exact name in [brackets] above.
- tool_name is the full tool name exactly as listed.
- Arguments must be valid JSON.
- Every MCP tool invocation MUST use TOOL_CALL: format.
- Do NOT explain tool calls — just output them."#, example)
    } else {
        String::new()
    };

    // Check if this is an explanation request
    let prompt_lower = prompt.to_lowercase();
    let is_explanation = prompt_lower.contains("explain")
        || prompt_lower.contains("what does")
        || prompt_lower.contains("how does")
        || prompt_lower.contains("describe")
        || prompt_lower.contains("analyze")
        || prompt_lower.contains("review");

    let has_text_attachments = attachments
        .iter()
        .any(|a| !a.mime_type.starts_with("image/") && a.mime_type != "application/pdf");

    let system_prompt = if is_explanation && has_text_attachments {
        "You are a helpful coding assistant. Explain code clearly and thoroughly. Describe what the code does, how it works, and any important patterns or concepts used. Be direct and informative.".to_string()
    } else {
        format!(
            r#"You are an expert terminal assistant in Drover. The user describes tasks in natural language and you respond with executable shell commands OR MCP tool calls.

Rules:
- Respond with ONLY executable shell commands (one per line) or TOOL_CALL directives.
- No explanations, no markdown, no code fences, no commentary.
- ALWAYS single-quote URLs that contain special shell characters (?, &, =, #, etc.).
- If a task is dangerous, respond with: ERROR: <reason>
- If the question is conversational or doesn't need commands, respond with: ANSWER: <your response>{}{}{}"#,
            context_block, tools_block, tool_instructions
        )
    };

    // Build Gemini contents
    let mut parts: Vec<GeminiPart> = Vec::new();

    // Add text prompt
    let mut user_text = prompt.clone();
    for attachment in &attachments {
        if !attachment.mime_type.starts_with("image/") && attachment.mime_type != "application/pdf" {
            user_text.push_str(&format!("\n\n--- {} ---\n{}", attachment.name, attachment.content));
        }
    }
    parts.push(GeminiPart::Text {
        text: user_text,
    });

    // Add image attachments as inline data
    for attachment in &attachments {
        if attachment.mime_type.starts_with("image/") {
            parts.push(GeminiPart::InlineData {
                inline_data: GeminiInlineData {
                    mime_type: attachment.mime_type.clone(),
                    data: attachment.content.clone(),
                },
            });
        }
    }

    let contents = vec![GeminiContent {
        role: "user".to_string(),
        parts,
    }];

    let reply = gemini_chat(&api_key, &model, &system_prompt, contents, 60).await?;

    // Prefix explanation responses with ANSWER:
    if is_explanation && has_text_attachments && !reply.starts_with("ANSWER:") {
        Ok(format!("ANSWER: {}", reply))
    } else {
        Ok(reply)
    }
}

#[tauri::command]
pub async fn ai_gemini_edit_code(
    api_key: String,
    model: String,
    file_name: String,
    file_path: Option<String>,
    file_content: String,
    instruction: String,
) -> Result<CodeEditResult, String> {
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

    let contents = vec![GeminiContent {
        role: "user".to_string(),
        parts: vec![GeminiPart::Text { text: user_content }],
    }];

    let modified = gemini_chat(&api_key, &model, system_prompt, contents, 60).await?;

    let diff_lines = generate_diff(&file_content, &modified);
    let summary = generate_edit_summary(&file_content, &modified, &instruction);

    Ok(CodeEditResult {
        file_name,
        file_path,
        original: file_content,
        modified,
        diff_lines,
        summary,
    })
}

#[tauri::command]
pub async fn ai_gemini_edit_code_from_path(
    api_key: String,
    model: String,
    file_path: String,
    instruction: String,
    cwd: Option<String>,
) -> Result<CodeEditResult, String> {
    use std::path::PathBuf;

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    let expand_tilde = |p: &str| -> String {
        if p.starts_with('~') {
            p.replacen('~', &home, 1)
        } else {
            p.to_string()
        }
    };

    let resolved_path = if file_path.starts_with('/') || file_path.contains(':') {
        file_path.clone()
    } else if file_path.starts_with('~') {
        expand_tilde(&file_path)
    } else if let Some(ref cwd) = cwd {
        let expanded_cwd = expand_tilde(cwd);
        let mut path = PathBuf::from(&expanded_cwd);
        path.push(&file_path);
        path.to_string_lossy().to_string()
    } else {
        file_path.clone()
    };

    let file_content = std::fs::read_to_string(&resolved_path)
        .map_err(|e| format!("Failed to read file '{}': {}", resolved_path, e))?;

    let file_name = PathBuf::from(&resolved_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.clone());

    ai_gemini_edit_code(api_key, model, file_name, Some(resolved_path), file_content, instruction).await
}

// ── Gemini image generation (Nano Banana 2) ───────────────────────────────────

#[derive(Serialize)]
pub struct ImageGenerationResult {
    pub image_data: String,  // base64-encoded image
    pub mime_type: String,
    pub text: String,        // any accompanying text from the model
}

#[tauri::command]
pub async fn ai_gemini_generate_image(
    api_key: String,
    model: String,
    prompt: String,
) -> Result<ImageGenerationResult, String> {
    let body = GeminiRequest {
        contents: vec![GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart::Text { text: prompt }],
        }],
        system_instruction: None,
        generation_config: Some(GeminiGenerationConfig {
            response_modalities: Some(vec!["TEXT".to_string(), "IMAGE".to_string()]),
        }),
    };

    let url = format!(
        "{}/models/{}:generateContent?key={}",
        GEMINI_API_BASE, model, api_key
    );

    let client = build_http_client(120)?;
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini image request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini API error {}: {}", status, text));
    }

    let parsed: GeminiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

    if let Some(error) = parsed.error {
        return Err(format!("Gemini error: {}", error.message));
    }

    let parts = parsed
        .candidates
        .and_then(|c| c.into_iter().next())
        .map(|c| c.content.parts)
        .ok_or_else(|| "Gemini returned no content".to_string())?;

    let mut image_data = String::new();
    let mut mime_type = String::from("image/png");
    let mut text_parts = Vec::new();

    for part in parts {
        if let Some(inline) = part.inline_data {
            image_data = inline.data;
            mime_type = inline.mime_type;
        }
        if let Some(t) = part.text {
            text_parts.push(t);
        }
    }

    if image_data.is_empty() {
        return Err("Gemini did not return an image. Try rephrasing your prompt.".to_string());
    }

    Ok(ImageGenerationResult {
        image_data,
        mime_type,
        text: text_parts.join("\n"),
    })
}

#[tauri::command]
pub async fn ai_chat(
    base_url: String,
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

    let result = kuratchi_chat(
        &base_url,
        &api_token,
        None,
        Some(DEFAULT_KURATCHI_MODEL.to_string()),
        messages,
        30,
    ).await?;

    Ok(result.reply)
}

#[derive(Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub output: String,
}

#[tauri::command]
pub async fn ai_summarize(
    base_url: String,
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

    let result = kuratchi_chat(
        &base_url,
        &api_token,
        None,
        Some(DEFAULT_KURATCHI_MODEL.to_string()),
        messages,
        30,
    ).await?;

    Ok(result.reply)
}

#[tauri::command]
pub async fn ai_generate_session_title(
    base_url: String,
    api_token: String,
    prompt: String,
    model: Option<String>,
) -> Result<String, String> {
    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: MessageContent::Text(
                "Generate a short descriptive chat title for a developer's request. Return only the title, with no quotes, no markdown, and no punctuation unless required. Keep it under 6 words."
                    .to_string(),
            ),
        },
        AiMessage {
            role: "user".to_string(),
            content: MessageContent::Text(prompt),
        },
    ];

    let result = kuratchi_chat(
        &base_url,
        &api_token,
        None,
        model.or(Some(DEFAULT_KURATCHI_MODEL.to_string())),
        messages,
        20,
    )
    .await?;

    Ok(result.reply.trim().trim_matches('"').to_string())
}

#[tauri::command]
pub async fn ai_list_sessions(
    base_url: String,
    api_token: String,
) -> Result<Vec<KuratchiSessionSummary>, String> {
    let client = build_http_client(30)?;
    let resp = client
        .get(build_kuratchi_url(&base_url, "/ai/sessions"))
        .header("Authorization", format!("Bearer {}", api_token))
        .send()
        .await
        .map_err(|e| format!("List sessions request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("List sessions error {}: {}", status, text));
    }

    let parsed: KuratchiSessionsEnvelope = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse sessions response: {}", e))?;

    if !parsed.success {
        return Err(parsed.error.unwrap_or_else(|| "List sessions failed".to_string()));
    }

    Ok(parsed.data.unwrap_or_default())
}

#[tauri::command]
pub async fn ai_create_session(
    base_url: String,
    api_token: String,
    title: Option<String>,
    model: Option<String>,
) -> Result<KuratchiSessionSummary, String> {
    kuratchi_create_session(&base_url, &api_token, title, model).await
}

#[tauri::command]
pub async fn ai_delete_session(
    base_url: String,
    api_token: String,
    session_id: String,
) -> Result<(), String> {
    let client = build_http_client(30)?;
    let resp = client
        .delete(build_kuratchi_url(&base_url, &format!("/ai/sessions/{}", session_id)))
        .header("Authorization", format!("Bearer {}", api_token))
        .send()
        .await
        .map_err(|e| format!("Delete session request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Delete session error {}: {}", status, text));
    }

    Ok(())
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
    base_url: String,
    api_token: String,
    model: String,
    session_id: Option<String>,
    prompt: String,
    attachments: Vec<AiAttachment>,
    cwd: Option<String>,
    os_info: Option<String>,
    shell: Option<String>,
    ssh_target: Option<String>,
) -> Result<AiChatTurn, String> {
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

    let _system_prompt = format!(
        r#"You are an expert terminal assistant in Drover. The user describes tasks in natural language and you respond with executable shell commands OR MCP tool calls.

Rules:
- Respond with ONLY executable shell commands (one per line) or TOOL_CALL directives.
- No explanations, no markdown, no code fences, no commentary.
- ALWAYS single-quote URLs that contain special shell characters (?, &, =, #, etc.). For example: curl 'https://wttr.in/Houston?format=j1'
- If a task is dangerous, respond with: ERROR: <reason>{}{}{}"#,
        context_block, tools_block, &tool_instructions
    );

    if attachments
        .iter()
        .any(|attachment| attachment.mime_type.starts_with("image/"))
    {
        return Err("Kuratchi AI sessions do not support image attachments yet".to_string());
    }

    process_without_images(
        &base_url,
        &api_token,
        &model,
        session_id,
        &prompt,
        &attachments,
        &context_block,
        &tools_block,
        &tool_instructions,
    )
    .await
}

async fn process_without_images(
    base_url: &str,
    api_token: &str,
    model: &str,
    session_id: Option<String>,
    prompt: &str,
    attachments: &[AiAttachment],
    context_block: &str,
    tools_block: &str,
    tool_instructions: &str,
) -> Result<AiChatTurn, String> {
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
        return process_explanation_request(base_url, api_token, model, session_id, prompt, attachments).await;
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

    let result = kuratchi_chat(
        base_url,
        api_token,
        session_id,
        Some(model.to_string()),
        messages,
        60,
    )
    .await?;

    Ok(AiChatTurn {
        session_id: result.session_id,
        response: result.reply,
    })
}

async fn process_explanation_request(
    base_url: &str,
    api_token: &str,
    model: &str,
    session_id: Option<String>,
    prompt: &str,
    attachments: &[AiAttachment],
) -> Result<AiChatTurn, String> {
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

    let result = kuratchi_chat(
        base_url,
        api_token,
        session_id,
        Some(model.to_string()),
        messages,
        60,
    )
    .await?;
    
    // Return with ANSWER: prefix so frontend displays it directly
    Ok(AiChatTurn {
        session_id: result.session_id,
        response: format!("ANSWER: {}", result.reply),
    })
}

#[derive(Deserialize)]
pub struct FailedAttempt {
    pub command: String,
    pub error: String,
}

#[tauri::command]
pub async fn ai_retry(
    base_url: String,
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

    Ok(
        kuratchi_chat(
            &base_url,
            &api_token,
            None,
            Some(DEFAULT_KURATCHI_MODEL.to_string()),
            messages,
            60,
        )
        .await?
        .reply,
    )
}

#[tauri::command]
pub async fn ai_summarize_tool_results(
    base_url: String,
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

    Ok(
        kuratchi_chat(
            &base_url,
            &api_token,
            None,
            Some(DEFAULT_KURATCHI_MODEL.to_string()),
            messages,
            30,
        )
        .await?
        .reply,
    )
}

#[tauri::command]
pub async fn ai_edit_code(
    base_url: String,
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

    let modified = kuratchi_chat(
        &base_url,
        &api_token,
        None,
        Some(model),
        messages,
        60,
    )
    .await?
    .reply;

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
    base_url: String,
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
        base_url,
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
