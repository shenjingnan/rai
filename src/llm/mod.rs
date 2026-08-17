/// 本地 LLM 模块。
///
/// 分层：
/// - `LlmEngine`（门面）：生命周期 + worker 线程 + 命令/事件 channel，供 CLI/Tauri 使用。
/// - `LlmProvider`（trait）：后端抽象，本地 llama.cpp 只是其中一种实现。
/// - `local`：`LocalLlamaProvider`，唯一接触 llama.cpp 的地方。
pub mod agent;
pub mod config;
pub mod error;
pub mod http;
pub mod local;
pub mod provider;
pub mod tools;
pub mod types;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use agent::Agent;
use config::ResolvedLlmConfig;
use error::LlmError;
use tools::ToolRuntime;
use types::{FinishReason, GenParams, InputItem, OutputItem, TokenDelta};

/// LLM 引擎事件（worker 线程 → 调用方）。
#[derive(Debug, PartialEq)]
pub enum LlmEvent {
    /// 一次文本增量
    Token(TokenDelta),
    /// 生成结束（含结束原因）
    Finished(FinishReason),
    /// 错误（含中文描述）
    Error(String),
    /// 加载/卸载后的状态变化
    Status { ready: bool },
}

enum LlmCommand {
    Load,
    Unload,
    Generate {
        input: Vec<InputItem>,
        params: GenParams,
        cancel: Arc<AtomicBool>,
    },
    Shutdown,
}

/// 带模型 identity 的加载错误（用于 `RuntimeStatus::LoadFailed` 精确匹配，避免 stale）。
#[derive(Debug, Clone)]
pub struct LlmLoadError {
    pub model_path: std::path::PathBuf,
    pub message: String,
}

/// LLM 引擎门面。
///
/// 内部 spawn 一个专用 worker OS 线程持有 `Box<dyn LlmProvider>`（llama.cpp 的
/// `LlamaContext` 非线程安全，必须在单线程内使用），命令经 `cmd_tx` 投递，
/// 结果经 `evt_rx` 流式返回。这与项目现有 `std::thread::spawn + mpsc + Arc<AtomicBool>`
/// 的模式（`src/kws/mod.rs`）一致。
///
/// RuntimeActual：`loaded_path` 只在 worker `Load` 成功后置位（带模型 identity），
/// 卸载/失败清空；`last_load_error` 记录「哪个模型」加载失败。
pub struct LlmEngine {
    cmd_tx: Sender<LlmCommand>,
    evt_rx: Mutex<Receiver<LlmEvent>>,
    handle: Mutex<Option<JoinHandle<()>>>,
    ready: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    loaded_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    last_load_error: Arc<Mutex<Option<LlmLoadError>>>,
}

impl LlmEngine {
    pub fn new(config: ResolvedLlmConfig) -> Result<Self, LlmError> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        let ready = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let loaded_path = Arc::new(Mutex::new(None));
        let last_load_error = Arc::new(Mutex::new(None));

        let ready_clone = ready.clone();
        let loaded_clone = loaded_path.clone();
        let error_clone = last_load_error.clone();
        let handle = std::thread::Builder::new()
            .name("llm-worker".to_string())
            .spawn(move || {
                worker_loop(
                    config,
                    cmd_rx,
                    evt_tx,
                    ready_clone,
                    loaded_clone,
                    error_clone,
                )
            })
            .map_err(|e| LlmError::BackendUnavailable(e.to_string()))?;

        Ok(Self {
            cmd_tx,
            evt_rx: Mutex::new(evt_rx),
            handle: Mutex::new(Some(handle)),
            ready,
            cancel,
            loaded_path,
            last_load_error,
        })
    }

    /// 模型是否已加载。
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// 当前实际加载的模型路径（`None` = 未加载）。
    pub fn loaded_model_path(&self) -> Option<std::path::PathBuf> {
        self.loaded_path.lock().ok().and_then(|g| g.clone())
    }

    /// 最近一次加载失败（带模型 identity），成功后清空。
    pub fn last_load_error(&self) -> Option<LlmLoadError> {
        self.last_load_error.lock().ok().and_then(|g| g.clone())
    }

    /// 阻塞等待加载完成（模型库切换事务用）。
    ///
    /// 入队 `Load` 后等待 worker 发出 `Status{ready:true}` 或 `Error`；`timeout` 只是
    /// 错误检测的安全网——即使超时，调用方 drop 本引擎时 `Drop` 会 `join` worker，
    /// 保证加载任务彻底结束、资源释放后才返回（见 `Drop` 注释）。
    pub fn load_blocking(&self, timeout: std::time::Duration) -> Result<(), String> {
        self.cmd_tx
            .send(LlmCommand::Load)
            .map_err(|_| "LLM worker 线程已退出".to_string())?;
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err("LLM 模型加载超时".to_string());
            }
            let remaining = deadline.saturating_duration_since(now);
            let recv = self
                .evt_rx
                .lock()
                .map_err(|_| "LLM 事件通道不可用".to_string())?
                .recv_timeout(remaining);
            match recv {
                Ok(LlmEvent::Status { ready: true }) => return Ok(()),
                Ok(LlmEvent::Status { ready: false }) => {
                    return Err("LLM 模型加载失败".to_string());
                }
                Ok(LlmEvent::Error(e)) => return Err(e),
                Ok(_) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    return Err("LLM 模型加载超时".to_string());
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("LLM worker 线程已退出".to_string());
                }
            }
        }
    }

    /// 加载模型（异步：结果经 [`LlmEvent::Status`]/[`LlmEvent::Error`] 返回）。
    pub fn load(&self) -> Result<(), LlmError> {
        self.cmd_tx
            .send(LlmCommand::Load)
            .map_err(|_| LlmError::BackendUnavailable("LLM worker 线程已退出".to_string()))
    }

    /// 卸载模型并释放内存。
    pub fn unload(&self) -> Result<(), LlmError> {
        self.cmd_tx
            .send(LlmCommand::Unload)
            .map_err(|_| LlmError::BackendUnavailable("LLM worker 线程已退出".to_string()))
    }

    /// 发起一次流式生成（异步：结果经 [`LlmEvent::Token`] 返回）。
    pub fn generate(&self, input: Vec<InputItem>, params: GenParams) -> Result<(), LlmError> {
        self.cancel.store(false, Ordering::Relaxed);
        self.cmd_tx
            .send(LlmCommand::Generate {
                input,
                params,
                cancel: self.cancel.clone(),
            })
            .map_err(|_| LlmError::BackendUnavailable("LLM worker 线程已退出".to_string()))
    }

    /// 取消当前生成。
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// 非阻塞拉取一个事件（由调用方线程轮询并转发，例如 Tauri `emit`）。
    pub fn try_recv(&self) -> Option<LlmEvent> {
        self.evt_rx.lock().ok()?.try_recv().ok()
    }
}

impl Drop for LlmEngine {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(LlmCommand::Shutdown);
        if let Some(handle) = self.handle.lock().ok().and_then(|mut h| h.take()) {
            let _ = handle.join();
        }
    }
}

/// 根据配置创建 provider。
///
/// 本地 llama.cpp（"local"）与 OpenAI 兼容 Responses（"openai" / "llamacpp-server"）
/// 共用同一 `LlmProvider` 抽象；Chat Completions fallback 留待后续。
pub fn create_provider(
    config: ResolvedLlmConfig,
) -> Result<Box<dyn provider::LlmProvider>, LlmError> {
    match config.provider.as_str() {
        "local" => Ok(Box::new(local::LocalLlamaProvider::new(config)?)),
        "openai" | "llamacpp-server" => Ok(Box::new(http::OpenAIResponsesProvider::new(&config)?)),
        other => Err(LlmError::UnsupportedProvider(other.to_string())),
    }
}

/// worker 线程主循环：创建 provider 后处理命令，直到 `Shutdown` 或 channel 关闭。
fn worker_loop(
    config: ResolvedLlmConfig,
    cmd_rx: Receiver<LlmCommand>,
    evt_tx: Sender<LlmEvent>,
    ready: Arc<AtomicBool>,
    loaded_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    last_load_error: Arc<Mutex<Option<LlmLoadError>>>,
) {
    let model_path = config.model_path.clone();
    let mut provider = match create_provider(config) {
        Ok(p) => p,
        Err(e) => {
            let _ = evt_tx.send(LlmEvent::Error(e.to_string()));
            return;
        }
    };
    let agent = Agent::new(ToolRuntime::new());

    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            LlmCommand::Load => match provider.load() {
                Ok(()) => {
                    ready.store(true, Ordering::Relaxed);
                    *loaded_path.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(model_path.clone());
                    *last_load_error.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    let _ = evt_tx.send(LlmEvent::Status { ready: true });
                }
                Err(e) => {
                    *loaded_path.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    *last_load_error.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(LlmLoadError {
                            model_path: model_path.clone(),
                            message: e.to_string(),
                        });
                    let _ = evt_tx.send(LlmEvent::Error(e.to_string()));
                }
            },
            LlmCommand::Unload => {
                provider.unload();
                ready.store(false, Ordering::Relaxed);
                *loaded_path.lock().unwrap_or_else(|e| e.into_inner()) = None;
                *last_load_error.lock().unwrap_or_else(|e| e.into_inner()) = None;
                let _ = evt_tx.send(LlmEvent::Status { ready: false });
            }
            LlmCommand::Generate {
                input,
                params,
                cancel,
            } => {
                let mut emit = |item: OutputItem| match item {
                    OutputItem::MessageDelta(delta) => {
                        let _ = evt_tx.send(LlmEvent::Token(delta));
                    }
                    OutputItem::ToolCall(_) => {
                        // 工具调用由 Agent Loop 内部处理，不外传（未来可发 llm-tool-call 事件）
                    }
                };
                match agent.run(&mut *provider, &input, &params, &mut emit, cancel) {
                    Ok(reason) => {
                        let _ = evt_tx.send(LlmEvent::Finished(reason));
                    }
                    Err(e) => {
                        let _ = evt_tx.send(LlmEvent::Error(e.to_string()));
                    }
                }
            }
            LlmCommand::Shutdown => break,
        }
    }

    provider.unload();
    ready.store(false, Ordering::Relaxed);
    *loaded_path.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
