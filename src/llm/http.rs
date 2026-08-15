/// OpenAI 兼容 Responses API 的 HTTP provider。
///
/// 通过 `POST {base_url}/v1/responses` 调用，SSE 流式解析。
/// 可用于 OpenAI 官方 API，或任何兼容 `/v1/responses` 的 server（如较新版本的 llama.cpp `llama-server`）。
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::llm::config::ResolvedLlmConfig;
use crate::llm::error::LlmError;
use crate::llm::provider::LlmProvider;
use crate::llm::types::{
    ChatRole, FinishReason, GenParams, InputItem, OutputItem, TokenDelta, ToolCall, ToolDefinition,
};

pub struct OpenAIResponsesProvider {
    client: reqwest::blocking::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
    system_prompt: String,
}

impl OpenAIResponsesProvider {
    pub fn new(config: &ResolvedLlmConfig) -> Result<Self, LlmError> {
        let base_url = config
            .base_url
            .clone()
            .ok_or_else(|| LlmError::BackendUnavailable("未配置 base_url".to_string()))?;
        let model = config
            .model
            .clone()
            .ok_or_else(|| LlmError::BackendUnavailable("未配置模型名（model）".to_string()))?;
        Ok(Self {
            client: reqwest::blocking::Client::new(),
            base_url,
            api_key: config.api_key.clone(),
            model,
            system_prompt: config.system_prompt.clone(),
        })
    }

    fn role_str(role: ChatRole) -> &'static str {
        match role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => "tool",
        }
    }

    /// 把 `InputItem` 转成 Responses API 的 `input` 数组。
    fn to_input(input: &[InputItem]) -> Vec<serde_json::Value> {
        input
            .iter()
            .map(|item| match item {
                InputItem::Message(m) => serde_json::json!({
                    "type": "message",
                    "role": Self::role_str(m.role),
                    "content": m.content,
                }),
                InputItem::ToolCall(c) => serde_json::json!({
                    "type": "function_call",
                    "name": c.name,
                    "arguments": c.arguments,
                    "call_id": c.id,
                }),
                InputItem::ToolResult(t) => serde_json::json!({
                    "type": "function_call_output",
                    "call_id": t.id,
                    "output": t.content,
                }),
            })
            .collect()
    }

    /// 把 `ToolDefinition` 转成 Responses API 的 `tools` 数组。
    fn to_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            })
            .collect()
    }
}

impl LlmProvider for OpenAIResponsesProvider {
    fn is_ready(&self) -> bool {
        true
    }

    fn load(&mut self) -> Result<(), LlmError> {
        // HTTP provider 无需本地加载。
        Ok(())
    }

    fn unload(&mut self) {}

    fn generate(
        &mut self,
        input: &[InputItem],
        tools: &[ToolDefinition],
        params: &GenParams,
        emit: &mut dyn FnMut(OutputItem),
        cancel: Arc<AtomicBool>,
    ) -> Result<FinishReason, LlmError> {
        // 1. 注入 system prompt（若调用方未提供）
        let mut full: Vec<serde_json::Value> = Vec::with_capacity(input.len() + 1);
        let has_system = input
            .iter()
            .any(|item| matches!(item, InputItem::Message(m) if m.role == ChatRole::System));
        if !has_system {
            full.push(serde_json::json!({
                "type": "message",
                "role": "system",
                "content": self.system_prompt,
            }));
        }
        full.extend(Self::to_input(input));

        // 2. 构造 Responses API 请求体
        let body = serde_json::json!({
            "model": self.model,
            "input": full,
            "tools": Self::to_tools(tools),
            "stream": true,
            "max_output_tokens": params.max_tokens,
            "temperature": params.temperature,
            "top_p": params.top_p,
        });

        // 3. 发送请求
        let url = format!("{}/responses", self.base_url.trim_end_matches('/'));
        let mut req = self.client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req
            .send()
            .map_err(|e| LlmError::BackendUnavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(LlmError::BackendUnavailable(format!(
                "HTTP {status}: {text}"
            )));
        }

        // 4. SSE 流式解析
        let reader = BufReader::new(resp);
        // function_call 状态机：跟踪当前工具调用的 name + arguments
        let mut current_fn: Option<(String, String)> = None;
        for line in reader.lines() {
            if cancel.load(Ordering::Relaxed) {
                return Ok(FinishReason::Cancelled);
            }
            let line = line.map_err(|e| LlmError::InferenceFailed(e.to_string()))?;
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data == "[DONE]" {
                break;
            }
            let Ok(event) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            match event["type"].as_str() {
                Some("response.output_text.delta") => {
                    if let Some(delta) = event["delta"].as_str().filter(|s| !s.is_empty()) {
                        emit(OutputItem::MessageDelta(TokenDelta::new(delta)));
                    }
                }
                Some("response.output_item.added") => {
                    if event["item"]["type"].as_str() == Some("function_call") {
                        let name = event["item"]["name"].as_str().unwrap_or("").to_string();
                        current_fn = Some((name, String::new()));
                    }
                }
                Some("response.function_call_arguments.delta") => {
                    if let (Some(delta), Some((_, args))) =
                        (event["delta"].as_str(), current_fn.as_mut())
                    {
                        args.push_str(delta);
                    }
                }
                Some("response.function_call_arguments.done") => {
                    if let Some((name, args)) = current_fn.take() {
                        let id = event["call_id"].as_str().map(str::to_string);
                        emit(OutputItem::ToolCall(ToolCall {
                            name,
                            arguments: args,
                            id,
                        }));
                    }
                }
                Some("response.completed") => return Ok(FinishReason::Eos),
                Some("response.failed") => {
                    let msg = event["error"]["message"].as_str().unwrap_or("unknown");
                    return Err(LlmError::InferenceFailed(msg.to_string()));
                }
                _ => {}
            }
        }

        Ok(FinishReason::Eos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{ChatMessage, ChatRole};

    #[test]
    fn test_to_input_converts_message_and_tool_result() {
        let input = vec![
            InputItem::Message(ChatMessage::new(ChatRole::User, "你好")),
            InputItem::ToolResult(crate::llm::types::ToolResult {
                id: "call_1".into(),
                name: "get_time".into(),
                content: "12:00".into(),
            }),
        ];
        let result = OpenAIResponsesProvider::to_input(&input);
        assert_eq!(result[0]["type"], "message");
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "你好");
        assert_eq!(result[1]["type"], "function_call_output");
        assert_eq!(result[1]["call_id"], "call_1");
    }

    #[test]
    fn test_to_tools_converts_definition() {
        let tools = vec![ToolDefinition {
            name: "get_time".into(),
            description: "获取当前时间".into(),
            parameters: serde_json::json!({"type": "object"}),
        }];
        let result = OpenAIResponsesProvider::to_tools(&tools);
        assert_eq!(result[0]["type"], "function");
        assert_eq!(result[0]["name"], "get_time");
        assert_eq!(result[0]["parameters"]["type"], "object");
    }
}
