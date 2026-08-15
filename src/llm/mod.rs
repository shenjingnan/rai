/// 本地 LLM 模块。
///
/// 分层：
/// - `LlmEngine`（门面）：生命周期 + worker 线程 + 命令/事件 channel，供 CLI/Tauri 使用。
/// - `LlmProvider`（trait）：后端抽象，本地 llama.cpp 只是其中一种实现。
/// - `local`：`LocalLlamaProvider`，唯一接触 llama.cpp 的地方。
pub mod config;
pub mod error;
pub mod local;
pub mod provider;
pub mod types;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use config::ResolvedLlmConfig;
use error::LlmError;
use types::{ChatMessage, FinishReason, GenParams, TokenDelta};

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
        messages: Vec<ChatMessage>,
        params: GenParams,
        cancel: Arc<AtomicBool>,
    },
    Shutdown,
}

/// LLM 引擎门面。
///
/// 内部 spawn 一个专用 worker OS 线程持有 `Box<dyn LlmProvider>`（llama.cpp 的
/// `LlamaContext` 非线程安全，必须在单线程内使用），命令经 `cmd_tx` 投递，
/// 结果经 `evt_rx` 流式返回。这与项目现有 `std::thread::spawn + mpsc + Arc<AtomicBool>`
/// 的模式（`src/kws/mod.rs`）一致。
pub struct LlmEngine {
    cmd_tx: Sender<LlmCommand>,
    evt_rx: Mutex<Receiver<LlmEvent>>,
    handle: Mutex<Option<JoinHandle<()>>>,
    ready: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl LlmEngine {
    pub fn new(config: ResolvedLlmConfig) -> Result<Self, LlmError> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        let ready = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));

        let ready_clone = ready.clone();
        let handle = std::thread::Builder::new()
            .name("llm-worker".to_string())
            .spawn(move || worker_loop(config, cmd_rx, evt_tx, ready_clone))
            .map_err(|e| LlmError::BackendUnavailable(e.to_string()))?;

        Ok(Self {
            cmd_tx,
            evt_rx: Mutex::new(evt_rx),
            handle: Mutex::new(Some(handle)),
            ready,
            cancel,
        })
    }

    /// 模型是否已加载。
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
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

    /// 发起一次流式生成（异步：token 经 [`LlmEvent::Token`] 返回）。
    pub fn generate(&self, messages: Vec<ChatMessage>, params: GenParams) -> Result<(), LlmError> {
        self.cancel.store(false, Ordering::Relaxed);
        self.cmd_tx
            .send(LlmCommand::Generate {
                messages,
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

/// worker 线程主循环：创建 provider 后处理命令，直到 `Shutdown` 或 channel 关闭。
fn worker_loop(
    config: ResolvedLlmConfig,
    cmd_rx: Receiver<LlmCommand>,
    evt_tx: Sender<LlmEvent>,
    ready: Arc<AtomicBool>,
) {
    let mut provider = match local::create_provider(config) {
        Ok(p) => p,
        Err(e) => {
            let _ = evt_tx.send(LlmEvent::Error(e.to_string()));
            return;
        }
    };

    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            LlmCommand::Load => match provider.load() {
                Ok(()) => {
                    ready.store(true, Ordering::Relaxed);
                    let _ = evt_tx.send(LlmEvent::Status { ready: true });
                }
                Err(e) => {
                    let _ = evt_tx.send(LlmEvent::Error(e.to_string()));
                }
            },
            LlmCommand::Unload => {
                provider.unload();
                ready.store(false, Ordering::Relaxed);
                let _ = evt_tx.send(LlmEvent::Status { ready: false });
            }
            LlmCommand::Generate {
                messages,
                params,
                cancel,
            } => {
                let mut emit = |delta: TokenDelta| {
                    let _ = evt_tx.send(LlmEvent::Token(delta));
                };
                match provider.generate(&messages, &params, &mut emit, cancel) {
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
}
