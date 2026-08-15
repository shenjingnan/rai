/// 工具运行时：注册工具定义 + 执行工具调用。
///
/// 第一版只实现一个 `get_current_time` 工具，用于验证 Agent Loop；
/// 未来在此扩展 `open_application` / `read_clipboard` 等桌面 Agent 工具。
use crate::llm::error::LlmError;
use crate::llm::types::ToolDefinition;

/// 工具运行时（第一版无状态，硬编码工具列表）。
#[derive(Default)]
pub struct ToolRuntime;

impl ToolRuntime {
    pub fn new() -> Self {
        Self
    }

    /// 可用工具的定义（传给模型的 `tools` 参数）。
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "get_current_time".to_string(),
            description: "获取当前本地时间（ISO 8601 格式）。".to_string(),
            parameters: serde_json::json!({ "type": "object", "properties": {} }),
        }]
    }

    /// 执行工具，返回文本结果。
    pub fn execute(&self, name: &str, arguments: &str) -> Result<String, LlmError> {
        let _ = arguments; // get_current_time 无参数
        match name {
            "get_current_time" => Ok(chrono::Local::now().to_rfc3339()),
            other => Err(LlmError::InferenceFailed(format!("未知工具: {other}"))),
        }
    }
}
