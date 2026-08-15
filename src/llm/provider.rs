use std::sync::Arc;
/// LLM 后端统一抽象。
///
/// 本地 llama.cpp 只是其中一种实现，未来可扩展 Ollama / OpenAI 兼容 / 云端等 provider。
/// 第一版保持最小，不过度抽象。
///
/// 注意：trait **不加 `Send` 约束**——llama.cpp 的 `LlamaContext` 非线程安全，
/// provider 实例在专用 worker 线程内创建并使用，不跨线程移动。
use std::sync::atomic::AtomicBool;

use crate::llm::error::LlmError;
use crate::llm::types::{ChatMessage, FinishReason, GenParams, TokenDelta};

pub trait LlmProvider {
    /// 模型是否已加载可用。
    fn is_ready(&self) -> bool;

    /// 加载模型（local 实现真正加载；云端实现可空操作返回 `Ok(())`）。
    fn load(&mut self) -> Result<(), LlmError>;

    /// 卸载模型并释放内存。
    fn unload(&mut self);

    /// 流式生成：逐 token 调用 `emit` 推送增量，最后返回结束原因。
    ///
    /// `cancel` 置位后应尽快停止并返回 [`FinishReason::Cancelled`]。
    /// 调用方负责把 [`FinishReason`] 作为生成结束信号转发给上层。
    fn generate(
        &mut self,
        messages: &[ChatMessage],
        params: &GenParams,
        emit: &mut dyn FnMut(TokenDelta),
        cancel: Arc<AtomicBool>,
    ) -> Result<FinishReason, LlmError>;
}
