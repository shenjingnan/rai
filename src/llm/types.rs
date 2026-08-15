/// LLM 模块的公共数据类型。
///
/// 这些类型是「owned + Serialize」的，用于在 Rust Core 与 Tauri 前端之间跨线程/跨进程传递，
/// 与 `kws::reaction::KwsResult` / `asr::reaction::AsrResult` 的约定一致——不泄漏 llama-cpp 类型。
use serde::{Deserialize, Serialize};

/// 对话消息角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 一条对话消息。
///
/// `tool_calls` 为后续 Tool Calling 预留（第一版不使用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl ChatMessage {
    pub fn new(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: None,
        }
    }
}

/// 一次工具调用（预留，第一版不实现）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    /// JSON 编码的参数
    pub arguments: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// 一次流式生成的文本增量（可能是半个字 / 半个词）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TokenDelta {
    pub text: String,
    pub is_final: bool,
}

impl TokenDelta {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_final: false,
        }
    }

    pub fn final_(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_final: true,
        }
    }
}

/// 一次生成结束的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FinishReason {
    /// 模型自然结束（命中 EOS / stop token）
    Eos,
    /// 达到 `max_tokens` 上限
    MaxTokens,
    /// 被用户取消
    Cancelled,
    /// 出错终止
    Error,
}

/// 生成采样参数。
///
/// 默认值对齐方案 §9.2 的「桌面 AI 伙伴」推荐（Qwen3 官方推荐 + 微调）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenParams {
    /// 上下文窗口大小（token）
    pub context_size: usize,
    /// 单次 decode 的 batch 大小
    pub batch_size: usize,
    /// 最多生成的 token 数
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub min_p: f32,
    pub repeat_penalty: f32,
    /// 随机种子；0 表示随机
    pub seed: u32,
    /// CPU 线程数；0 表示自动（物理核数 - 2，由 `config::resolve` 计算）
    pub threads: i32,
    /// 卸载到 GPU 的层数；-1 表示全部（Metal），0 表示纯 CPU。
    ///
    /// 默认 0（纯 CPU）：llama-cpp-2 0.1.154 内置 llama.cpp 的 Metal 后端在获取
    /// logits 时存在 `GGML_ASSERT(buf_dst)` 崩溃，Metal 加速留待 Phase 7 升级依赖后再启用。
    pub gpu_layers: i32,
    /// 是否开启 Qwen3 的思考模式（输出 `<think>` 块）。
    ///
    /// 默认 false：桌面伙伴场景下思考过程是噪音且增加首包延迟，关闭更自然。
    pub enable_thinking: bool,
}

impl Default for GenParams {
    fn default() -> Self {
        Self {
            context_size: 8192,
            batch_size: 512,
            max_tokens: 512,
            temperature: 0.7,
            top_p: 0.8,
            top_k: 20,
            min_p: 0.05,
            repeat_penalty: 1.05,
            seed: 0,
            threads: 0,
            gpu_layers: 0,
            enable_thinking: false,
        }
    }
}

/// 工具调用结果（对应 OpenAI Responses 的 `function_call_output` item）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 关联的 tool call id
    pub id: String,
    pub name: String,
    /// 工具执行的文本结果
    pub content: String,
}

/// 工具定义（供 Tool Calling 传给模型）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema 参数
    pub parameters: serde_json::Value,
}

/// LLM 输入项（一次 Agent 步的上下文，有序）。
///
/// 统一抽象：Responses API 的 `input` 与 Chat Completions 的 `messages` 都映射到它。
#[derive(Debug, Clone)]
pub enum InputItem {
    /// 一条聊天消息（system / user / assistant）
    Message(ChatMessage),
    /// assistant 的一次工具调用（对应 Responses 的 `function_call`，回填到 input）
    ToolCall(ToolCall),
    /// 一次工具调用结果（对应 `function_call_output`）
    ToolResult(ToolResult),
}

/// LLM 输出项（流式，逐 item 产出）。
#[derive(Debug, Clone)]
pub enum OutputItem {
    /// 文本增量（最终回复 / reasoning 内容）
    MessageDelta(TokenDelta),
    /// 一次工具调用请求
    ToolCall(ToolCall),
}
