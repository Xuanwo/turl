use std::path::Path;

use serde_json::Value;

use crate::error::{Result, TurlError};
use crate::model::{MessageRole, ProviderKind, ThreadMessage};
use crate::uri::ThreadUri;

const TOOL_TYPES: &[&str] = &[
    "tool_call",
    "tool_result",
    "tool_use",
    "function_call",
    "function_result",
    "function_response",
];

pub fn render_markdown(uri: &ThreadUri, source_path: &Path, raw_jsonl: &str) -> Result<String> {
    let messages = extract_messages(uri.provider, source_path, raw_jsonl)?;

    let mut output = String::new();
    output.push_str("# Thread\n\n");
    output.push_str(&format!("- URI: `{}`\n", uri.as_string()));
    output.push_str(&format!("- Source: `{}`\n\n", source_path.display()));

    if messages.is_empty() {
        output.push_str("_No user/assistant messages found._\n");
        return Ok(output);
    }

    for (idx, message) in messages.iter().enumerate() {
        let title = match message.role {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
        };

        output.push_str(&format!("## {}. {}\n\n", idx + 1, title));
        output.push_str(message.text.trim());
        output.push_str("\n\n");
    }

    Ok(output)
}

pub fn extract_messages(
    provider: ProviderKind,
    path: &Path,
    raw_jsonl: &str,
) -> Result<Vec<ThreadMessage>> {
    if provider == ProviderKind::Amp {
        return extract_amp_messages(path, raw_jsonl);
    }

    let mut messages = Vec::new();

    for (line_idx, line) in raw_jsonl.lines().enumerate() {
        let line_no = line_idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value = serde_json::from_str::<Value>(trimmed).map_err(|source| {
            TurlError::InvalidJsonLine {
                path: path.to_path_buf(),
                line: line_no,
                source,
            }
        })?;

        let extracted = match provider {
            ProviderKind::Amp => None,
            ProviderKind::Codex => extract_codex_message(&value),
            ProviderKind::Claude => extract_claude_message(&value),
            ProviderKind::Opencode => extract_opencode_message(&value),
        };

        if let Some(message) = extracted {
            messages.push(message);
        }
    }

    Ok(messages)
}

fn extract_amp_messages(path: &Path, raw_json: &str) -> Result<Vec<ThreadMessage>> {
    let value =
        serde_json::from_str::<Value>(raw_json).map_err(|source| TurlError::InvalidJsonLine {
            path: path.to_path_buf(),
            line: 1,
            source,
        })?;

    let mut messages = Vec::new();
    for message in value
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(role) = message
            .get("role")
            .and_then(Value::as_str)
            .and_then(parse_role)
        else {
            continue;
        };

        let text = extract_amp_text(message.get("content"));
        if text.trim().is_empty() {
            continue;
        }

        messages.push(ThreadMessage { role, text });
    }

    Ok(messages)
}

fn extract_codex_message(value: &Value) -> Option<ThreadMessage> {
    let record_type = value.get("type").and_then(Value::as_str)?;

    if record_type == "response_item" {
        let payload = value.get("payload")?;
        let payload_type = payload.get("type").and_then(Value::as_str)?;
        if payload_type != "message" {
            return None;
        }

        let role = payload.get("role").and_then(Value::as_str)?;
        let role = parse_role(role)?;
        let text = extract_text(payload.get("content"));
        if text.trim().is_empty() {
            return None;
        }

        return Some(ThreadMessage { role, text });
    }

    if record_type == "event_msg"
        && value
            .get("payload")
            .and_then(|payload| payload.get("type"))
            .and_then(Value::as_str)
            .is_some_and(|t| t == "agent_message")
    {
        let text = value
            .get("payload")
            .and_then(|payload| payload.get("message"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        if text.trim().is_empty() {
            return None;
        }

        return Some(ThreadMessage {
            role: MessageRole::Assistant,
            text,
        });
    }

    None
}

fn extract_claude_message(value: &Value) -> Option<ThreadMessage> {
    let record_type = value.get("type").and_then(Value::as_str)?;
    if record_type != "user" && record_type != "assistant" {
        return None;
    }

    let message = value.get("message")?;
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .or(Some(record_type))?;
    let role = parse_role(role)?;

    let text = extract_text(message.get("content"));
    if text.trim().is_empty() {
        return None;
    }

    Some(ThreadMessage { role, text })
}

fn extract_opencode_message(value: &Value) -> Option<ThreadMessage> {
    let record_type = value.get("type").and_then(Value::as_str)?;
    if record_type != "message" {
        return None;
    }

    let message = value.get("message")?;
    let role = message.get("role").and_then(Value::as_str)?;
    let role = parse_role(role)?;

    let mut chunks = Vec::new();
    for part in value
        .get("parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(part_type) = part.get("type").and_then(Value::as_str) else {
            continue;
        };

        if part_type != "text" && part_type != "reasoning" {
            continue;
        }

        if let Some(text) = part.get("text").and_then(Value::as_str)
            && !text.trim().is_empty()
        {
            chunks.push(text.trim().to_string());
        }
    }

    if chunks.is_empty() {
        return None;
    }

    Some(ThreadMessage {
        role,
        text: chunks.join("\n\n"),
    })
}

fn extract_amp_text(content: Option<&Value>) -> String {
    let Some(items) = content.and_then(Value::as_array) else {
        return String::new();
    };

    let mut chunks = Vec::new();
    for item in items {
        let Some(item_type) = item.get("type").and_then(Value::as_str) else {
            continue;
        };

        match item_type {
            "text" => {
                if let Some(text) = item.get("text").and_then(Value::as_str)
                    && !text.trim().is_empty()
                {
                    chunks.push(text.trim().to_string());
                }
            }
            "thinking" => {
                if let Some(thinking) = item.get("thinking").and_then(Value::as_str)
                    && !thinking.trim().is_empty()
                {
                    chunks.push(thinking.trim().to_string());
                }
            }
            _ => {}
        }
    }

    chunks.join("\n\n")
}

fn parse_role(role: &str) -> Option<MessageRole> {
    match role {
        "user" => Some(MessageRole::User),
        "assistant" => Some(MessageRole::Assistant),
        _ => None,
    }
}

fn extract_text(content: Option<&Value>) -> String {
    let Some(content) = content else {
        return String::new();
    };

    if let Some(text) = content.as_str() {
        return text.to_string();
    }

    let Some(items) = content.as_array() else {
        return String::new();
    };

    let mut chunks = Vec::new();

    for item in items {
        if let Some(item_type) = item.get("type").and_then(Value::as_str)
            && TOOL_TYPES.contains(&item_type)
        {
            continue;
        }

        if let Some(text) = item.get("text").and_then(Value::as_str)
            && !text.trim().is_empty()
        {
            chunks.push(text.trim().to_string());
            continue;
        }

        if let Some(text) = item.get("input_text").and_then(Value::as_str)
            && !text.trim().is_empty()
        {
            chunks.push(text.trim().to_string());
            continue;
        }

        if let Some(text) = item.get("output_text").and_then(Value::as_str)
            && !text.trim().is_empty()
        {
            chunks.push(text.trim().to_string());
        }
    }

    chunks.join("\n\n")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::model::ProviderKind;
    use crate::render::extract_messages;

    #[test]
    fn codex_filters_function_calls() {
        let raw = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}
{"type":"response_item","payload":{"type":"function_call","name":"ls"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"world"}]}}"#;

        let messages =
            extract_messages(ProviderKind::Codex, Path::new("/tmp/mock"), raw).expect("extract");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "hello");
        assert_eq!(messages[1].text, "world");
    }

    #[test]
    fn claude_filters_tool_use() {
        let raw = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hello"}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"search"},{"type":"text","text":"done"}]}}"#;

        let messages =
            extract_messages(ProviderKind::Claude, Path::new("/tmp/mock"), raw).expect("extract");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].text, "done");
    }

    #[test]
    fn opencode_extracts_text_and_reasoning_parts() {
        let raw = r#"{"type":"session","sessionId":"ses_43a90e3adffejRgrTdlJa48CtE"}
{"type":"message","id":"msg_1","sessionId":"ses_43a90e3adffejRgrTdlJa48CtE","message":{"role":"user","time":{"created":1}},"parts":[{"type":"text","text":"hello"}]}
{"type":"message","id":"msg_2","sessionId":"ses_43a90e3adffejRgrTdlJa48CtE","message":{"role":"assistant","time":{"created":2}},"parts":[{"type":"reasoning","text":"thinking"},{"type":"tool","tool":"read"},{"type":"text","text":"world"}]}"#;

        let messages =
            extract_messages(ProviderKind::Opencode, Path::new("/tmp/mock"), raw).expect("extract");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "hello");
        assert_eq!(messages[1].text, "thinking\n\nworld");
    }

    #[test]
    fn amp_extracts_text_and_thinking_content() {
        let raw = r#"{"id":"T-019c0797-c402-7389-bd80-d785c98df295","messages":[{"role":"user","content":[{"type":"text","text":"hello"}]},{"role":"assistant","content":[{"type":"thinking","thinking":"step by step"},{"type":"tool_use","name":"finder"},{"type":"text","text":"done"}]},{"role":"user","content":[{"type":"tool_result","toolUseID":"tool_1","run":{"status":"done","result":"ignored"}}]}]}"#;

        let messages =
            extract_messages(ProviderKind::Amp, Path::new("/tmp/mock"), raw).expect("extract");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "hello");
        assert_eq!(messages[1].text, "step by step\n\ndone");
    }
}
