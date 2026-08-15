/// LLM 模块的错误类型。
///
/// 公开模块边界用明确的 `LlmError` 枚举（而非到处 `anyhow!("...")`），
/// 便于调用方（CLI / Tauri 命令）把错误映射成中文友好提示返回给用户。
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// 模型文件不存在
    #[error("LLM 模型文件不存在：{0}")]
    ModelNotFound(PathBuf),

    /// 模型文件格式错误（非 GGUF 或文件损坏）
    #[error("模型文件格式不正确（非 GGUF 或文件损坏）：{0}")]
    InvalidModel(PathBuf),

    /// 不支持的模型架构/格式
    #[error("不支持的模型格式或架构：{0}")]
    UnsupportedModel(String),

    /// 模型加载失败
    #[error("模型加载失败：{0}")]
    LoadFailed(String),

    /// 内存不足
    #[error("内存不足，无法加载模型：{0}")]
    OutOfMemory(String),

    /// 推理失败
    #[error("推理失败：{0}")]
    InferenceFailed(String),

    /// 输入超出 context 长度
    #[error("上下文溢出：输入超出 context 长度")]
    ContextOverflow,

    /// 生成被取消
    #[error("生成已取消")]
    GenerationCancelled,

    /// 后端不可用
    #[error("后端不可用：{0}")]
    BackendUnavailable(String),

    /// 模型尚未加载
    #[error("模型尚未加载")]
    NotLoaded,

    /// 已在进行中的生成任务
    #[error("已在进行中的生成任务")]
    Busy,

    /// 不支持的 provider
    #[error("不支持的 LLM provider：{0}")]
    UnsupportedProvider(String),
}
