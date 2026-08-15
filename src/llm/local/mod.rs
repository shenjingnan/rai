/// 本地 LLM provider 实现（llama.cpp）。
pub mod llama;

pub use llama::LocalLlamaProvider;

use crate::llm::config::ResolvedLlmConfig;
use crate::llm::error::LlmError;
use crate::llm::provider::LlmProvider;

/// 根据配置创建 provider（第一版仅支持 `"local"`，未来扩展 Ollama/OpenAI 等）。
pub fn create_provider(config: ResolvedLlmConfig) -> Result<Box<dyn LlmProvider>, LlmError> {
    match config.provider.as_str() {
        "local" => Ok(Box::new(LocalLlamaProvider::new(config)?)),
        other => Err(LlmError::UnsupportedProvider(other.to_string())),
    }
}
