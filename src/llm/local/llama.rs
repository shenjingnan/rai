/// `LocalLlamaProvider`：封装 llama-cpp-2，是项目中唯一直接接触 llama.cpp 的地方。
///
/// 设计要点：
/// - llama.cpp 的 `LlamaContext` 持有对 `LlamaModel` 的引用（自引用），因此模型用
///   `Box::leak` 拿到 `'static` 引用，卸载时按「先 context 后 model」的顺序手动释放。
/// - 所有推理都在持有本实例的 worker 线程内同步执行，不跨线程移动（`LlamaContext` 非线程安全）。
use std::io::Read;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::gguf::GgufContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use crate::llm::config::ResolvedLlmConfig;
use crate::llm::error::LlmError;
use crate::llm::provider::LlmProvider;
use crate::llm::types::{
    ChatMessage, ChatRole, FinishReason, GenParams, InputItem, OutputItem, TokenDelta,
    ToolDefinition,
};

/// 本地 llama.cpp provider。
pub struct LocalLlamaProvider {
    backend: LlamaBackend,
    /// `'static` 引用（由 `Box::leak` 取得，`unload` 时 `Box::from_raw` 释放）。
    model: Option<&'static LlamaModel>,
    context: Option<LlamaContext<'static>>,
    config: ResolvedLlmConfig,
}

impl LocalLlamaProvider {
    pub fn new(config: ResolvedLlmConfig) -> Result<Self, LlmError> {
        Ok(Self {
            backend: LlamaBackend::init()
                .map_err(|e| LlmError::BackendUnavailable(e.to_string()))?,
            model: None,
            context: None,
            config,
        })
    }

    /// 读取 GGUF metadata 里的字符串值（如 `general.architecture`）。
    fn gguf_meta(path: &Path, key: &str) -> Option<String> {
        let ctx = GgufContext::from_file(path)?;
        let idx = ctx.find_key(key);
        if idx < 0 {
            return None;
        }
        ctx.val_str(idx).map(str::to_string)
    }

    /// 校验 GGUF：扩展名 + 文件头 magic（只读前 4 字节）。
    fn check_gguf(path: &Path) -> bool {
        if path.extension().and_then(|e| e.to_str()) != Some("gguf") {
            return false;
        }
        let Ok(mut f) = std::fs::File::open(path) else {
            return false;
        };
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic).is_ok() && &magic == b"GGUF"
    }

    /// 模型架构名（供展示/错误提示）。
    pub fn architecture(&self) -> String {
        Self::gguf_meta(&self.config.model_path, "general.architecture")
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// 构建 chat template：优先 GGUF 内嵌模板，兜底 llama.cpp 内置 `qwen3`。
    fn chat_template(model: &LlamaModel) -> Result<LlamaChatTemplate, LlmError> {
        model
            .chat_template(None)
            .or_else(|_| model.chat_template(Some("qwen3")))
            .map_err(|_| LlmError::UnsupportedModel("模型缺少 chat template".to_string()))
    }
}

impl LlmProvider for LocalLlamaProvider {
    fn is_ready(&self) -> bool {
        self.model.is_some() && self.context.is_some()
    }

    fn load(&mut self) -> Result<(), LlmError> {
        if self.is_ready() {
            return Ok(());
        }

        let path = &self.config.model_path;
        // 1. 存在性检查
        if !path.is_file() {
            return Err(LlmError::ModelNotFound(path.clone()));
        }
        // 2. 格式检查（GGUF magic）
        if !Self::check_gguf(path) {
            return Err(LlmError::InvalidModel(path.clone()));
        }

        // 3. 加载模型（gpu_layers < 0 表示「全部 offload」，用足够大的值让 llama.cpp clamp 到实际层数）
        let gpu_layers = self.config.params.gpu_layers;
        let n_gpu_layers = if gpu_layers < 0 {
            999
        } else {
            gpu_layers as u32
        };
        let model_params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
        let model = LlamaModel::load_from_file(&self.backend, path, &model_params)
            .map_err(|e| LlmError::LoadFailed(e.to_string()))?;
        let model: &'static LlamaModel = Box::leak(Box::new(model));

        // 4. 创建 context
        let p = &self.config.params;
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(p.context_size as u32))
            .with_n_batch(p.batch_size as u32)
            .with_n_threads(p.threads)
            .with_n_threads_batch(p.threads);
        let context = match model.new_context(&self.backend, ctx_params) {
            Ok(ctx) => ctx,
            Err(e) => {
                // context 创建失败时释放已泄漏的 model，避免内存泄漏
                unsafe {
                    drop(Box::from_raw(model as *const LlamaModel as *mut LlamaModel));
                }
                return Err(LlmError::LoadFailed(e.to_string()));
            }
        };

        self.model = Some(model);
        self.context = Some(context);
        tracing::info!(architecture = %self.architecture(), "LLM 模型已加载");
        Ok(())
    }

    fn unload(&mut self) {
        // 顺序：先 drop context（借用 model），再释放 model。
        self.context = None;
        if let Some(model) = self.model.take() {
            unsafe {
                drop(Box::from_raw(model as *const LlamaModel as *mut LlamaModel));
            }
        }
    }

    fn generate(
        &mut self,
        input: &[InputItem],
        tools: &[ToolDefinition],
        params: &GenParams,
        emit: &mut dyn FnMut(OutputItem),
        cancel: Arc<AtomicBool>,
    ) -> Result<FinishReason, LlmError> {
        if !self.is_ready() {
            return Err(LlmError::NotLoaded);
        }
        let model = self.model.expect("is_ready 已检查");
        let context = self.context.as_mut().expect("is_ready 已检查");

        // 本地 llama.cpp 第一版只支持消息（不支持 tool result / tool calling）。
        // 把 InputItem 展平成 ChatMessage，ToolResult 暂被忽略，tools 暂不使用。
        let _ = tools;
        let messages: Vec<ChatMessage> = input
            .iter()
            .filter_map(|item| match item {
                InputItem::Message(m) => Some(m.clone()),
                InputItem::ToolCall(_) | InputItem::ToolResult(_) => None,
            })
            .collect();

        // 1. 注入 system prompt（若调用方未提供）
        let mut full: Vec<ChatMessage> = Vec::with_capacity(messages.len() + 1);
        if !messages.iter().any(|m| m.role == ChatRole::System) {
            full.push(ChatMessage::new(
                ChatRole::System,
                self.config.system_prompt.clone(),
            ));
        }
        full.extend_from_slice(&messages);

        // 2. 构造 chat template prompt
        let tmpl = Self::chat_template(model)?;
        let chat: Vec<LlamaChatMessage> = full
            .iter()
            .map(|m| LlamaChatMessage::new(role_str(m.role), m.content.clone()))
            .collect::<Result<_, _>>()
            .map_err(|e| LlmError::InferenceFailed(e.to_string()))?;
        let mut prompt = model
            .apply_chat_template(&tmpl, &chat, true)
            .map_err(|e| LlmError::InferenceFailed(e.to_string()))?;

        // 关闭 Qwen3 思考模式：在 assistant 引导后追加空 <think> 块，让模型直接作答、不输出思考过程。
        if !params.enable_thinking {
            prompt.push_str("<think>\n\n</think>\n\n");
        }

        // 3. tokenize
        let tokens = model
            .str_to_token(&prompt, AddBos::Never)
            .map_err(|e| LlmError::InferenceFailed(e.to_string()))?;

        // 4. context 溢出检查
        if tokens.is_empty() {
            return Err(LlmError::InferenceFailed("tokenize 结果为空".to_string()));
        }
        if tokens.len() + params.max_tokens > context.n_ctx() as usize {
            return Err(LlmError::ContextOverflow);
        }

        // 4.5 清空 KV cache：每次 generate 是独立对话，避免上次生成残留的 KV 导致 pos 冲突
        context.clear_kv_cache();

        // 5. decode prompt
        let mut batch = LlamaBatch::new(tokens.len() + params.max_tokens, 1);
        batch
            .add_sequence(&tokens, 0, false)
            .map_err(|e| LlmError::InferenceFailed(e.to_string()))?;
        context
            .decode(&mut batch)
            .map_err(|e| LlmError::InferenceFailed(e.to_string()))?;
        batch.clear();

        // 6. 采样链
        let mut sampler = build_sampler(&*context, params);

        // 7. 逐 token 生成
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut n_generated = 0usize;
        let mut pos = tokens.len() as i32;
        // 第一次采样取 prompt 最后一个 token 的 logits；后续循环取单 token batch 的 idx 0。
        let mut sample_idx = (tokens.len() - 1) as i32;

        loop {
            if cancel.load(Ordering::Relaxed) {
                return Ok(FinishReason::Cancelled);
            }

            let token = sampler.sample(&*context, sample_idx);
            sampler.accept(token);

            if model.is_eog_token(token) {
                return Ok(FinishReason::Eos);
            }

            let piece = model
                .token_to_piece(token, &mut decoder, false, None)
                .map_err(|e| LlmError::InferenceFailed(e.to_string()))?;
            if !piece.is_empty() {
                emit(OutputItem::MessageDelta(TokenDelta::new(piece)));
            }

            n_generated += 1;
            if n_generated >= params.max_tokens {
                return Ok(FinishReason::MaxTokens);
            }

            batch
                .add(token, pos, &[0], true)
                .map_err(|e| LlmError::InferenceFailed(e.to_string()))?;
            context
                .decode(&mut batch)
                .map_err(|e| LlmError::InferenceFailed(e.to_string()))?;
            batch.clear();
            pos += 1;
            sample_idx = 0;
        }
    }
}

impl Drop for LocalLlamaProvider {
    fn drop(&mut self) {
        self.unload();
    }
}

/// 把 `ChatRole` 映射为 llama.cpp chat template 的 role 字符串。
fn role_str(role: ChatRole) -> String {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::Tool => "tool",
    }
    .to_string()
}

/// 构建采样链：penalties → temp → top_k → top_p → min_p → dist/greedy。
fn build_sampler(context: &LlamaContext, params: &GenParams) -> LlamaSampler {
    let mut samplers = vec![LlamaSampler::penalties(
        context.n_ctx() as i32,
        params.repeat_penalty,
        0.0,
        0.0,
    )];

    if params.temperature <= 0.0 {
        samplers.push(LlamaSampler::greedy());
    } else {
        samplers.push(LlamaSampler::temp(params.temperature));
        if params.top_k > 0 {
            samplers.push(LlamaSampler::top_k(params.top_k as i32));
        }
        samplers.push(LlamaSampler::top_p(params.top_p, 1));
        if params.min_p > 0.0 {
            samplers.push(LlamaSampler::min_p(params.min_p, 1));
        }
        samplers.push(LlamaSampler::dist(params.seed));
    }

    LlamaSampler::chain_simple(samplers)
}

/// 供测试/工具使用的 GGUF 校验入口。
pub fn is_gguf_file(path: &Path) -> bool {
    LocalLlamaProvider::check_gguf(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::LlmProvider;
    use crate::llm::types::{
        ChatMessage, ChatRole, FinishReason, GenParams, InputItem, OutputItem,
    };
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    #[test]
    #[ignore = "需要 ~/.zapmomo/models 下的真实 GGUF 模型"]
    fn test_two_consecutive_generations() {
        let cfg = crate::llm::config::resolve(None, None).unwrap();
        let mut provider = LocalLlamaProvider::new(cfg).unwrap();
        provider.load().unwrap();

        for round in 0..2 {
            let input = vec![InputItem::Message(ChatMessage::new(
                ChatRole::User,
                "请只回复一个字",
            ))];
            let cancel = Arc::new(AtomicBool::new(false));
            let mut text = String::new();
            let result = provider
                .generate(
                    &input,
                    &[],
                    &GenParams::default(),
                    &mut |item| {
                        if let OutputItem::MessageDelta(delta) = item {
                            text.push_str(&delta.text);
                        }
                    },
                    cancel,
                )
                .unwrap();
            assert!(
                matches!(result, FinishReason::Eos | FinishReason::MaxTokens),
                "第 {round} 轮未正常结束"
            );
            assert!(!text.is_empty(), "第 {round} 轮生成结果为空");
        }
    }
}
