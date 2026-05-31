use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::interactive::InteractiveConfig;
use crate::providers::ProviderKind;

pub fn normalize_provider_line(
    provider: ProviderKind,
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    match provider {
        ProviderKind::Codex => normalize_codex(value),
        ProviderKind::Grok => normalize_grok(value),
        ProviderKind::Kiro => normalize_kiro(value),
        ProviderKind::Agy => None, // Agy uses protobuf, not JSONL — output captured from PTY directly
        _ => None,
    }
}

/// Codex rollout JSONL normalization.
/// Format: {"timestamp":..., "type":"response_item"|"event_msg"|"session_meta", "payload":{...}}
/// payload.type: "message" (role=assistant/user), "function_call", "function_call_output", "reasoning"
fn normalize_codex(value: &serde_json::Value) -> Option<serde_json::Value> {
    let outer_type = value.get("type")?.as_str()?;
    match outer_type {
        "response_item" => {
            let payload = value.get("payload")?;
            let payload_type = payload.get("type")?.as_str()?;
            match payload_type {
                "message" => {
                    let role = payload.get("role")?.as_str()?;
                    match role {
                        "assistant" | "user" => {
                            let mut out = serde_json::json!({
                                "type": role,
                                "message": payload,
                            });
                            if let Some(id) = payload.get("id") {
                                out["session_id"] = id.clone();
                            }
                            Some(out)
                        }
                        _ => None, // developer, system prompts — skip
                    }
                }
                "function_call" => Some(serde_json::json!({
                    "type": "tool_use",
                    "message": payload,
                })),
                "function_call_output" => Some(serde_json::json!({
                    "type": "tool_result",
                    "message": payload,
                })),
                _ => None, // reasoning, etc — skip
            }
        }
        "event_msg" => {
            let payload = value.get("payload")?;
            let event_type = payload.get("type")?.as_str()?;
            if event_type == "task_completed" || event_type == "task_errored" {
                Some(serde_json::json!({
                    "type": "system",
                    "message": event_type,
                }))
            } else {
                None
            }
        }
        _ => None, // session_meta, etc
    }
}

/// Grok chat_history.jsonl normalization.
/// Format: {"type":"assistant"|"user"|"system"|"tool_use"|"tool_result", "content":...}
fn normalize_grok(value: &serde_json::Value) -> Option<serde_json::Value> {
    let record_type = value.get("type")?.as_str()?;
    match record_type {
        "assistant" => {
            let mut out = serde_json::json!({
                "type": "assistant",
                "message": value,
            });
            if let Some(id) = value.get("session_id").or_else(|| value.get("id")) {
                out["session_id"] = id.clone();
            }
            Some(out)
        }
        "user" => {
            // Skip system-injected user messages (system-reminder, user_info)
            let content = value.get("content")?;
            if let Some(text) = content.as_str() {
                if text.contains("<system-reminder>") || text.contains("<user_info>") {
                    return None;
                }
            }
            if let Some(blocks) = content.as_array() {
                if let Some(first) = blocks.first() {
                    if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                        if text.contains("<system-reminder>") || text.contains("<user_info>") {
                            return None;
                        }
                    }
                }
            }
            Some(serde_json::json!({
                "type": "user",
                "message": value,
            }))
        }
        "tool_use" => Some(serde_json::json!({
            "type": "tool_use",
            "message": value,
        })),
        "tool_result" => Some(serde_json::json!({
            "type": "tool_result",
            "message": value,
        })),
        "system" => None, // Skip system prompt records
        _ => None,
    }
}

/// Kiro conversations_v2 / chat output normalization.
/// Kiro --no-interactive outputs JSONL with {"type":"assistant","content":[{"type":"text","text":"..."}]}
fn normalize_kiro(value: &serde_json::Value) -> Option<serde_json::Value> {
    let msg_type = value.get("type")?.as_str()?;
    match msg_type {
        "assistant" | "user" => {
            let mut out = serde_json::json!({
                "type": msg_type,
                "message": value,
            });
            if let Some(id) = value
                .get("conversationId")
                .or_else(|| value.get("session_id"))
            {
                out["session_id"] = id.clone();
            }
            Some(out)
        }
        "tool_use" | "tool_result" => Some(serde_json::json!({
            "type": msg_type,
            "message": value,
        })),
        "system" | "error" => Some(serde_json::json!({
            "type": "system",
            "message": value
                .get("content")
                .or_else(|| value.get("message"))
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        })),
        _ => None,
    }
}

pub fn update_tool_state(value: &serde_json::Value, active_tools: &Arc<AtomicUsize>) {
    let record_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match record_type {
        "tool_use" => {
            active_tools.fetch_add(1, Ordering::Relaxed);
        }
        "tool_result" => {
            let prev = active_tools.load(Ordering::Relaxed);
            if prev > 0 {
                active_tools.fetch_sub(1, Ordering::Relaxed);
            }
        }
        "assistant" => {
            if let Some(content) = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
            {
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        active_tools.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        _ => {}
    }
}

pub fn extract_text_content(value: &serde_json::Value) -> Option<String> {
    let message = value.get("message")?;
    let content = message.get("content")?;

    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    if let Some(blocks) = content.as_array() {
        let mut text = String::new();
        for block in blocks {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if matches!(block_type, "text" | "output_text") {
                if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
            }
        }
        return Some(text);
    }

    None
}

pub fn emit_result(
    config: &InteractiveConfig,
    last_assistant: &Option<serde_json::Value>,
    stdout: &mut std::io::StdoutLock<'_>,
) {
    if config.output_format != "json" && config.output_format != "stream-json" {
        return;
    }
    let Some(assistant) = last_assistant else {
        return;
    };

    let result_text = extract_text_content(assistant).unwrap_or_default();
    let session_id = assistant
        .get("session_id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let model = assistant
        .pointer("/message/model")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let usage = assistant
        .pointer("/message/usage")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": result_text,
        "session_id": session_id,
        "model": model,
        "usage": usage,
    });

    if let Ok(json) = serde_json::to_string(&result) {
        let _ = writeln!(stdout, "{json}");
        let _ = stdout.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── normalize_codex ───

    #[test]
    fn codex_normalizes_assistant_message() {
        let value = serde_json::json!({
            "type": "response_item",
            "timestamp": 1234,
            "payload": {
                "type": "message",
                "role": "assistant",
                "id": "msg_001",
                "content": [{"type": "output_text", "text": "hello"}]
            }
        });
        let out = normalize_codex(&value).unwrap();
        assert_eq!(out["type"], "assistant");
        assert_eq!(out["session_id"], "msg_001");
    }

    #[test]
    fn codex_normalizes_function_call() {
        let value = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "function_call",
                "name": "read_file",
                "arguments": "{\"path\":\"src/lib.rs\"}"
            }
        });
        let out = normalize_codex(&value).unwrap();
        assert_eq!(out["type"], "tool_use");
    }

    #[test]
    fn codex_normalizes_function_call_output() {
        let value = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "function_call_output",
                "output": "file contents here"
            }
        });
        let out = normalize_codex(&value).unwrap();
        assert_eq!(out["type"], "tool_result");
    }

    #[test]
    fn codex_normalizes_task_completed() {
        let value = serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "task_completed"}
        });
        let out = normalize_codex(&value).unwrap();
        assert_eq!(out["type"], "system");
        assert_eq!(out["message"], "task_completed");
    }

    #[test]
    fn codex_skips_developer_role() {
        let value = serde_json::json!({
            "type": "response_item",
            "payload": {"type": "message", "role": "developer", "content": "system prompt"}
        });
        assert!(normalize_codex(&value).is_none());
    }

    #[test]
    fn codex_skips_reasoning() {
        let value = serde_json::json!({
            "type": "response_item",
            "payload": {"type": "reasoning", "content": "thinking..."}
        });
        assert!(normalize_codex(&value).is_none());
    }

    #[test]
    fn codex_skips_session_meta() {
        let value = serde_json::json!({"type": "session_meta", "payload": {}});
        assert!(normalize_codex(&value).is_none());
    }

    // ─── normalize_grok ───

    #[test]
    fn grok_normalizes_assistant() {
        let value = serde_json::json!({
            "type": "assistant",
            "content": [{"type": "text", "text": "hi"}],
            "session_id": "grok-sess-1"
        });
        let out = normalize_grok(&value).unwrap();
        assert_eq!(out["type"], "assistant");
        assert_eq!(out["session_id"], "grok-sess-1");
    }

    #[test]
    fn grok_normalizes_tool_use() {
        let value = serde_json::json!({
            "type": "tool_use",
            "name": "bash",
            "input": {"command": "ls"}
        });
        let out = normalize_grok(&value).unwrap();
        assert_eq!(out["type"], "tool_use");
    }

    #[test]
    fn grok_normalizes_tool_result() {
        let value = serde_json::json!({
            "type": "tool_result",
            "content": "output here"
        });
        let out = normalize_grok(&value).unwrap();
        assert_eq!(out["type"], "tool_result");
    }

    #[test]
    fn grok_filters_system_reminder_string() {
        let value = serde_json::json!({
            "type": "user",
            "content": "<system-reminder>You are a helpful assistant</system-reminder>"
        });
        assert!(normalize_grok(&value).is_none());
    }

    #[test]
    fn grok_filters_system_reminder_blocks() {
        let value = serde_json::json!({
            "type": "user",
            "content": [{"type": "text", "text": "<system-reminder>reminder</system-reminder>"}]
        });
        assert!(normalize_grok(&value).is_none());
    }

    #[test]
    fn grok_filters_user_info() {
        let value = serde_json::json!({
            "type": "user",
            "content": "<user_info>name: Jun</user_info>"
        });
        assert!(normalize_grok(&value).is_none());
    }

    #[test]
    fn grok_keeps_normal_user() {
        let value = serde_json::json!({
            "type": "user",
            "content": "hello grok"
        });
        let out = normalize_grok(&value).unwrap();
        assert_eq!(out["type"], "user");
    }

    #[test]
    fn grok_skips_system_type() {
        let value = serde_json::json!({"type": "system", "content": "system prompt"});
        assert!(normalize_grok(&value).is_none());
    }

    // ─── normalize_kiro ───

    #[test]
    fn kiro_normalizes_assistant() {
        let value = serde_json::json!({
            "type": "assistant",
            "content": [{"type": "text", "text": "response"}],
            "conversationId": "kiro-conv-1"
        });
        let out = normalize_kiro(&value).unwrap();
        assert_eq!(out["type"], "assistant");
        assert_eq!(out["session_id"], "kiro-conv-1");
    }

    #[test]
    fn kiro_normalizes_tool_use() {
        let value = serde_json::json!({
            "type": "tool_use",
            "name": "fs_read",
            "input": {}
        });
        let out = normalize_kiro(&value).unwrap();
        assert_eq!(out["type"], "tool_use");
    }

    #[test]
    fn kiro_normalizes_error() {
        let value = serde_json::json!({
            "type": "error",
            "message": "something went wrong"
        });
        let out = normalize_kiro(&value).unwrap();
        assert_eq!(out["type"], "system");
        assert_eq!(out["message"], "something went wrong");
    }

    #[test]
    fn kiro_normalizes_system_with_content() {
        let value = serde_json::json!({
            "type": "system",
            "content": "session started"
        });
        let out = normalize_kiro(&value).unwrap();
        assert_eq!(out["type"], "system");
        assert_eq!(out["message"], "session started");
    }

    #[test]
    fn kiro_skips_unknown_type() {
        let value = serde_json::json!({"type": "metadata", "data": {}});
        assert!(normalize_kiro(&value).is_none());
    }

    // ─── update_tool_state ───

    #[test]
    fn tool_state_increments_on_tool_use() {
        let counter = Arc::new(AtomicUsize::new(0));
        let value = serde_json::json!({"type": "tool_use"});
        update_tool_state(&value, &counter);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tool_state_decrements_on_tool_result() {
        let counter = Arc::new(AtomicUsize::new(2));
        let value = serde_json::json!({"type": "tool_result"});
        update_tool_state(&value, &counter);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tool_state_does_not_underflow() {
        let counter = Arc::new(AtomicUsize::new(0));
        let value = serde_json::json!({"type": "tool_result"});
        update_tool_state(&value, &counter);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn tool_state_tracks_content_block_tool_use() {
        let counter = Arc::new(AtomicUsize::new(0));
        let value = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "I'll read the file"},
                    {"type": "tool_use", "name": "read_file"}
                ]
            }
        });
        update_tool_state(&value, &counter);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    // ─── extract_text_content ───

    #[test]
    fn extracts_text_from_string_content() {
        let value = serde_json::json!({
            "type": "assistant",
            "message": {"content": "plain text response"}
        });
        assert_eq!(extract_text_content(&value).unwrap(), "plain text response");
    }

    #[test]
    fn extracts_text_from_content_blocks() {
        let value = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "hello "},
                    {"type": "tool_use", "name": "bash"},
                    {"type": "text", "text": "world"}
                ]
            }
        });
        assert_eq!(extract_text_content(&value).unwrap(), "hello world");
    }

    #[test]
    fn extracts_output_text_blocks() {
        let value = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [{"type": "output_text", "text": "codex output"}]
            }
        });
        assert_eq!(extract_text_content(&value).unwrap(), "codex output");
    }

    #[test]
    fn returns_none_without_message() {
        let value = serde_json::json!({"type": "system"});
        assert!(extract_text_content(&value).is_none());
    }

    // ─── normalize_provider_line routing ───

    #[test]
    fn routes_codex() {
        let value = serde_json::json!({
            "type": "response_item",
            "payload": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "ok"}]}
        });
        let out = normalize_provider_line(ProviderKind::Codex, &value);
        assert!(out.is_some());
        assert_eq!(out.unwrap()["type"], "assistant");
    }

    #[test]
    fn routes_grok() {
        let value = serde_json::json!({"type": "assistant", "content": "hi"});
        let out = normalize_provider_line(ProviderKind::Grok, &value);
        assert!(out.is_some());
    }

    #[test]
    fn routes_kiro() {
        let value =
            serde_json::json!({"type": "assistant", "content": [{"type": "text", "text": "hi"}]});
        let out = normalize_provider_line(ProviderKind::Kiro, &value);
        assert!(out.is_some());
    }

    #[test]
    fn returns_none_for_agy() {
        let value = serde_json::json!({"type": "assistant", "content": "hi"});
        assert!(normalize_provider_line(ProviderKind::Agy, &value).is_none());
    }
}
