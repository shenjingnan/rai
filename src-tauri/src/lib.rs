//! ZapMomo 桌面应用（Tauri 2）。
//!
//! 复用根 crate `zapmomo` 的 KWS / 音频 / 配置逻辑：
//! - 通过 Tauri command 暴露设备列表、KWS 配置、开始/停止监听；
//! - 监听循环跑在独立 `std::thread`，检测到唤醒词经 `TauriReaction`
//!   以 `kws-detected` 事件推给前端；结束（正常/出错/手动停止）发 `kws-stopped`。
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
#[cfg(target_os = "macos")]
use tauri::menu::PredefinedMenuItem;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use zapmomo::asr::config::AsrParamsPatch;
use zapmomo::asr::{AsrReaction, AsrResult};
use zapmomo::config::settings::{
    self, AsrSettings, CompanionWindowPosition, KwsSettings, Live2dSettings, LlmSettings,
    TtsSettings,
};
use zapmomo::datetime::iso_timestamp_now;
use zapmomo::kws::{KwsResult, Reaction, ReactionOutcome};
use zapmomo::llm::types::{ChatMessage, ChatRole, GenParams, InputItem, LlmParamsPatch};
use zapmomo::llm::{LlmEngine, LlmEvent};
use zapmomo::model_library;
use zapmomo::model_library::catalog::{CatalogPage, CatalogQuery, RemoteModelDetail};
use zapmomo::model_library::download::{
    DownloadArtifactRequest, DownloadConfig, DownloadEventSink, DownloadManager, DownloadTaskView,
    UreqFileDownloader,
};
use zapmomo::model_library::huggingface::HfApiClient;
use zapmomo::model_library::{
    InstallState as LibInstallState, LibraryModel, RuntimeAction as LibRuntimeAction,
    SetCurrentResult, SystemResources, registry::ModelType as LibModelType,
    storage::StorageInfoView,
};
use zapmomo::tts::config::TtsParamsPatch;
use zapmomo::voice::VoiceSession;
use zapmomo::voice::config::CliOverrides as VoiceCliOverrides;
use zapmomo::voice::events::VoiceEvent;
use zapmomo::voice::records;
use zapmomo::voice::state::SessionState as VoicePhase;

// 角色窗口的 macOS 非激活面板：点击/拖动不激活应用、不抢前台焦点，
// 使其表现为纯桌面摆件（参考 BongoCat 的 `tauri-nspanel` 方案）。
#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(CompanionPanel {
        config: {
            is_floating_panel: true,
            // 摆件无需键盘输入，彻底不抢焦点。
            can_become_key_window: false,
            // 关键：永不成为 main window，点击不会把焦点从上一个窗口抢过来。
            can_become_main_window: false,
        }
    })
}

/// 监听线程状态：共享停止标志 + 线程句柄 + 运行时实际模型目录（RuntimeActual）。
struct ListenState {
    running: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// 当前会话真正使用的模型目录（启动监听时固化；停止/线程退出时清空）
    active_model_dir: Arc<Mutex<Option<PathBuf>>>,
}

impl ListenState {
    fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
            active_model_dir: Arc::new(Mutex::new(None)),
        }
    }

    fn is_listening(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn active_model_dir(&self) -> Option<PathBuf> {
        self.active_model_dir.lock().ok().and_then(|g| g.clone())
    }
}

/// RAII：进入监听时置 `active_model_dir`，无论正常/错误/panic 退出监听线程都会清空。
struct ActiveModelGuard {
    target: Arc<Mutex<Option<PathBuf>>>,
}

impl ActiveModelGuard {
    fn set(target: &Arc<Mutex<Option<PathBuf>>>, path: PathBuf) -> Self {
        *target.lock().unwrap_or_else(|e| e.into_inner()) = Some(path);
        Self {
            target: target.clone(),
        }
    }
}

impl Drop for ActiveModelGuard {
    fn drop(&mut self) {
        *self.target.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// 语音会话线程状态：共享停止标志 + 线程句柄（仿 `ListenState`）。
struct VoiceSessionState {
    running: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// 当前会话的打断标志：会话线程创建 session 后写入，线程退出时清空。
    /// 全局快捷键「打断播报」置位 → 会话循环 `do_barge_in`（停生成/合成/播放，回 Armed）。
    barge_in: Mutex<Option<Arc<AtomicBool>>>,
}

impl VoiceSessionState {
    fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
            barge_in: Mutex::new(None),
        }
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

// ---- 语音会话事件载荷（emit 给前端）----

#[derive(Clone, Serialize)]
struct VoiceSessionStatePayload {
    running: bool,
    state: VoicePhase,
}

#[derive(Clone, Serialize)]
struct VoiceWakePayload {
    keyword: String,
}

#[derive(Clone, Serialize)]
struct VoiceTranscriptPayload {
    text: String,
    is_final: bool,
}

#[derive(Clone, Serialize)]
struct VoiceTokenPayload {
    delta: String,
}

#[derive(Clone, Serialize)]
struct VoiceReplyPayload {
    sentence: String,
}

#[derive(Clone, Serialize)]
struct VoicePlayPayload {
    sentence: String,
}

#[derive(Clone, Serialize)]
struct VoiceReplyFinishedPayload {
    reason: String,
    /// 该轮完整可见回复（`None` = 空回复），供前端提交对话记录
    text: Option<String>,
}

#[derive(Clone, Serialize)]
struct VoiceErrorPayload {
    message: String,
}

#[derive(Clone, Serialize)]
struct VoiceStoppedPayload {
    error: Option<String>,
}

/// 模型下载状态：防重入标志。
struct DownloadState {
    in_progress: Arc<AtomicBool>,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            in_progress: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// 下载进度事件载荷（推给前端）。
#[derive(Clone, Serialize)]
struct DownloadProgressPayload {
    stage: String,
    percent: f64,
    message: String,
}

/// 退出作用域（含 panic / 命令取消）时复位下载标志。
struct ResetOnDrop(Arc<AtomicBool>);

impl Drop for ResetOnDrop {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// 把唤醒词检测结果通过 Tauri 事件发给前端。
struct TauriReaction {
    app: AppHandle,
}

impl Reaction for TauriReaction {
    fn on_keyword(&mut self, result: &KwsResult) -> ReactionOutcome {
        let _ = self.app.emit("kws-detected", result);
        ReactionOutcome::Continue
    }
}

/// 监听结束事件载荷（正常停止时 `error` 为 `None`）。
#[derive(Clone, Serialize)]
struct ListenStopped {
    error: Option<String>,
}

#[derive(Serialize)]
struct AppInfo {
    version: String,
    product_name: String,
}

#[tauri::command]
fn get_app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        product_name: env!("CARGO_PKG_NAME").to_string(),
    }
}

/// 列出可用麦克风输入设备。
#[tauri::command]
fn list_devices() -> Vec<String> {
    zapmomo::audio::list_input_devices()
}

/// 请求 macOS 麦克风授权（触发系统授权弹窗）。返回是否已授权。
///
/// macOS 未授权时输入设备被系统隐藏、枚举为空，需先经此授权恢复；
/// 调试模式下每次重新编译授权会失效，前端在设备列表为空时引导用户点击。
#[tauri::command]
fn request_mic_permission() -> Result<bool, String> {
    zapmomo::audio::request_mic_permission()
}

/// GUI 展示用的 KWS 配置信息。
#[derive(Serialize)]
struct KwsConfigInfo {
    enabled: bool,
    custom_keywords: String,
    model_dir: String,
    provider: String,
    num_threads: i32,
    sample_rate: i32,
    chunk_size: usize,
    keywords_score: f32,
    keywords_threshold: f32,
    debug: bool,
    keywords: Vec<String>,
    models_present: bool,
    model_downloading: bool,
    settings_path: String,
}

/// `set_kws_params` 载荷：可调整的 KWS 引擎/运行参数（snake_case 直传，缺省项不修改）。
#[derive(Debug, Clone, Default, Deserialize)]
struct KwsParamsPatch {
    keywords_threshold: Option<f32>,
    keywords_score: Option<f32>,
    chunk_size: Option<usize>,
    num_threads: Option<i32>,
    debug: Option<bool>,
}

impl KwsParamsPatch {
    /// 先整体校验（任一越界立即 Err），再逐项写入 `KwsSettings`，保证出错时不部分修改。
    fn apply_to(&self, kws: &mut KwsSettings) -> Result<(), String> {
        if let Some(v) = self.keywords_threshold
            && !(0.0..=1.0).contains(&v)
        {
            return Err(format!("灵敏度/阈值需在 0~1，当前 {v}"));
        }
        if let Some(v) = self.keywords_score
            && !(0.1..=10.0).contains(&v)
        {
            return Err(format!("关键词加权需在 0.1~10，当前 {v}"));
        }
        if let Some(v) = self.chunk_size
            && !(400..=16_000).contains(&v)
        {
            return Err(format!("采样块大小需在 400~16000（@16k），当前 {v}"));
        }
        if let Some(v) = self.num_threads
            && !(1..=32).contains(&v)
        {
            return Err(format!("线程数需在 1~32，当前 {v}"));
        }

        if let Some(v) = self.keywords_threshold {
            kws.keywords_threshold = Some(v);
        }
        if let Some(v) = self.keywords_score {
            kws.keywords_score = Some(v);
        }
        if let Some(v) = self.chunk_size {
            kws.chunk_size = Some(v);
        }
        if let Some(v) = self.num_threads {
            kws.num_threads = Some(v);
        }
        if let Some(v) = self.debug {
            kws.debug = Some(v);
        }
        Ok(())
    }
}

/// 读取合并后的 KWS 配置（settings.toml + 默认值），并给出模型是否就绪。
#[tauri::command]
fn get_kws_config(state: State<'_, DownloadState>) -> Result<KwsConfigInfo, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let kws_settings = settings.as_ref().and_then(|s| s.kws.clone());
    let cfg = zapmomo::kws::config::resolve(kws_settings.as_ref(), None)?;

    let files = [
        &cfg.encoder,
        &cfg.decoder,
        &cfg.joiner,
        &cfg.tokens,
        &cfg.keywords_file,
    ];
    let models_present = files.iter().all(|p| p.is_file());
    let keywords =
        zapmomo::kws::config::parse_keywords_file(&cfg.keywords_file).unwrap_or_default();
    tracing::info!(
        "get_kws_config: settings.kws.enabled={:?} resolve.enabled={} models_present={} settings_path={}",
        kws_settings.as_ref().and_then(|k| k.enabled),
        cfg.enabled,
        models_present,
        zapmomo::config::settings::get_settings_path().display()
    );

    Ok(KwsConfigInfo {
        enabled: cfg.enabled,
        custom_keywords: kws_settings
            .as_ref()
            .and_then(|s| s.custom_keywords.clone())
            .unwrap_or_default(),
        model_dir: cfg.model_dir.display().to_string(),
        provider: cfg.provider.clone(),
        num_threads: cfg.num_threads,
        sample_rate: cfg.sample_rate,
        chunk_size: cfg.chunk_size,
        keywords_score: cfg.keywords_score,
        keywords_threshold: cfg.keywords_threshold,
        debug: cfg.debug,
        keywords,
        models_present,
        model_downloading: state.in_progress.load(Ordering::Relaxed),
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
    })
}

/// 开始实时监听唤醒词（command 与启动自动监听共用）。
///
/// 校验模型文件后启动独立线程跑 `run_realtime_with`，检测结果经
/// `kws-detected` 事件发给前端；线程结束发 `kws-stopped`。
fn start_listen_impl(
    app: AppHandle,
    state: &ListenState,
    device: Option<String>,
    keywords: Option<String>,
) -> Result<(), String> {
    if state.is_listening() {
        return Err("已在监听中".to_string());
    }

    let settings = zapmomo::config::settings::load_settings()?;
    let kws_settings = settings.as_ref().and_then(|s| s.kws.clone());
    let cfg = zapmomo::kws::config::resolve(kws_settings.as_ref(), None)?;

    // 同步校验/编码附加关键词（原始中文自动转 ppinyin），避免编码失败时空指针崩溃
    if let Some(k) = keywords.as_deref() {
        zapmomo::kws::token::encode_custom_keywords(k, &cfg.tokens)?;
    }

    // 预检模型文件，失败同步返回清晰错误（避免在后台线程里才报错）
    let files = [
        &cfg.encoder,
        &cfg.decoder,
        &cfg.joiner,
        &cfg.tokens,
        &cfg.keywords_file,
    ];
    if let Some(missing) = files.iter().find(|p| !p.is_file()) {
        return Err(format!(
            "缺少模型文件: {}\n\n请在「配置」面板点击「下载模型」，或运行 `zapmomo kws install-model` 下载模型。",
            missing.display()
        ));
    }

    let running = state.running.clone();
    running.store(true, Ordering::Relaxed);
    // RuntimeActual：记录本次会话使用的模型目录；随线程退出（RAII）自动清空
    let _active_guard = ActiveModelGuard::set(&state.active_model_dir, cfg.model_dir.clone());
    let thread_app = app.clone();
    let handle = std::thread::spawn(move || {
        let _active = _active_guard;
        tracing::info!("KWS listen thread started");
        let mut reaction = TauriReaction { app: thread_app };
        let result = zapmomo::kws::run_realtime_with(
            &cfg,
            device.as_deref(),
            None,
            keywords.as_deref(),
            &mut reaction,
            Some(&running),
        );
        running.store(false, Ordering::Relaxed);
        match &result {
            Ok(()) => tracing::info!("KWS listen thread finished (clean)"),
            Err(e) => tracing::error!("KWS listen thread finished with error: {e}"),
        }
        let payload = ListenStopped {
            error: result.err(),
        };
        let _ = reaction.app.emit("kws-stopped", payload);
    });
    *state.handle.lock().expect("listen handle lock poisoned") = Some(handle);
    // 通知前端监听已启动（含切换设备后的自动重启；启动瞬间前端未订阅时静默丢弃）
    let _ = app.emit("kws-started", ListenStopped { error: None });
    Ok(())
}

/// 开始实时监听唤醒词。 —— Tauri command 外壳，签名与前端契约不变。
#[tauri::command]
fn start_listen(
    app: AppHandle,
    state: State<'_, ListenState>,
    device: Option<String>,
    keywords: Option<String>,
) -> Result<(), String> {
    start_listen_impl(app, state.inner(), device, keywords)
}

/// 停止实时监听的内部实现（`stop_listen` command 与「切换设备重启」共用）。
fn stop_listen_inner(state: &ListenState) -> Result<(), String> {
    if !state.is_listening() {
        return Err("当前没有在监听".to_string());
    }
    state.running.store(false, Ordering::Relaxed);
    let handle = state
        .handle
        .lock()
        .expect("listen handle lock poisoned")
        .take();
    if let Some(handle) = handle {
        let _ = handle.join();
    }
    // RAII guard 在线程退出时已清空；这里兜底确保一致
    *state
        .active_model_dir
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

/// 停止实时监听：置停止标志并等待线程退出。
#[tauri::command]
fn stop_listen(state: State<'_, ListenState>) -> Result<(), String> {
    tracing::warn!("stop_listen called (is_listening={})", state.is_listening());
    stop_listen_inner(state.inner())
}

/// 当前是否正在监听。
#[tauri::command]
fn is_listening(state: State<'_, ListenState>) -> bool {
    state.is_listening()
}

/// 下载并安装 KWS 模型（默认安装到 `~/.zapmomo/models/<模型名>`）。
///
/// 防重入；下载在阻塞线程池执行，进度经 `kws-model-download-progress` 事件推给前端。
#[tauri::command]
async fn download_kws_model(app: AppHandle, state: State<'_, DownloadState>) -> Result<(), String> {
    let flag = state.in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("模型下载已在进行中，请稍候".to_string());
    }
    let dest = zapmomo::kws::model::user_model_dir();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ResetOnDrop(flag);
        let mut progress = |p: zapmomo::kws::model::DownloadProgress| {
            let stage = match p.stage {
                zapmomo::kws::model::DownloadStage::Downloading => "downloading",
                zapmomo::kws::model::DownloadStage::Verifying => "verifying",
                zapmomo::kws::model::DownloadStage::Extracting => "extracting",
                zapmomo::kws::model::DownloadStage::Done => "done",
            };
            let _ = app.emit(
                "kws-model-download-progress",
                DownloadProgressPayload {
                    stage: stage.to_string(),
                    percent: p.percent,
                    message: p.message,
                },
            );
        };
        zapmomo::kws::model::install_model_to(&dest, false, &mut progress)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("下载任务异常: {e}"))?
}

/// ASR 监听线程状态：共享停止标志 + 线程句柄。
struct AsrListenState {
    running: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// 当前会话真正使用的模型目录（RuntimeActual）
    active_model_dir: Arc<Mutex<Option<PathBuf>>>,
}

impl AsrListenState {
    fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
            active_model_dir: Arc::new(Mutex::new(None)),
        }
    }

    fn is_listening(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn active_model_dir(&self) -> Option<PathBuf> {
        self.active_model_dir.lock().ok().and_then(|g| g.clone())
    }
}

/// ASR 模型下载状态：防重入标志。
struct AsrDownloadState {
    in_progress: Arc<AtomicBool>,
}

impl Default for AsrDownloadState {
    fn default() -> Self {
        Self {
            in_progress: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// 把语音识别结果通过 Tauri 事件发给前端。
struct TauriAsrReaction {
    app: AppHandle,
}

impl AsrReaction for TauriAsrReaction {
    fn on_result(&mut self, result: &AsrResult) -> ReactionOutcome {
        let _ = self.app.emit("asr-result", result);
        ReactionOutcome::Continue
    }
}

/// GUI 展示用的 ASR 配置信息（含可经 `set_asr_params` 调整的引擎参数）。
#[derive(Serialize)]
struct AsrConfigInfo {
    enabled: bool,
    model_dir: String,
    provider: String,
    num_threads: i32,
    sample_rate: i32,
    chunk_size: usize,
    decoding_method: String,
    enable_endpoint: bool,
    rule1_min_trailing_silence: f32,
    rule2_min_trailing_silence: f32,
    rule3_min_utterance_length: f32,
    blank_penalty: f32,
    hotwords: Option<String>,
    enable_punctuation: bool,
    debug: bool,
    models_present: bool,
    punctuation_present: bool,
    model_downloading: bool,
    settings_path: String,
}

/// 读取合并后的 ASR 配置（settings.toml + 默认值），并给出模型是否就绪。
#[tauri::command]
fn get_asr_config(state: State<'_, AsrDownloadState>) -> Result<AsrConfigInfo, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
    let cfg = zapmomo::asr::config::resolve(asr_settings.as_ref(), None)?;

    let files = [&cfg.encoder, &cfg.decoder, &cfg.joiner, &cfg.tokens];
    let models_present = files.iter().all(|p| p.is_file());
    let punctuation_present = cfg.punctuation_model.is_file();
    tracing::info!(
        "get_asr_config: settings.asr.enabled={:?} resolve.enabled={} models_present={}",
        asr_settings.as_ref().and_then(|a| a.enabled),
        cfg.enabled,
        models_present
    );

    Ok(AsrConfigInfo {
        enabled: cfg.enabled,
        model_dir: cfg.model_dir.display().to_string(),
        provider: cfg.provider.clone(),
        num_threads: cfg.num_threads,
        sample_rate: cfg.sample_rate,
        chunk_size: cfg.chunk_size,
        decoding_method: cfg.decoding_method.clone(),
        enable_endpoint: cfg.enable_endpoint,
        rule1_min_trailing_silence: cfg.rule1_min_trailing_silence,
        rule2_min_trailing_silence: cfg.rule2_min_trailing_silence,
        rule3_min_utterance_length: cfg.rule3_min_utterance_length,
        blank_penalty: cfg.blank_penalty,
        hotwords: cfg.hotwords.clone(),
        enable_punctuation: cfg.enable_punctuation,
        debug: cfg.debug,
        models_present,
        punctuation_present,
        model_downloading: state.in_progress.load(Ordering::Relaxed),
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
    })
}

/// 开始实时语音识别的内部实现（`start_asr_listen` command 与「切换设备重启」共用）。
///
/// 校验模型文件后启动独立线程跑 `run_realtime_with`，识别结果经
/// `asr-result` 事件发给前端；线程结束发 `asr-stopped`，启动成功发 `asr-started`。
fn start_asr_listen_impl(
    app: AppHandle,
    state: &AsrListenState,
    device: Option<String>,
) -> Result<(), String> {
    if state.is_listening() {
        return Err("已在识别中".to_string());
    }

    let settings = zapmomo::config::settings::load_settings()?;
    let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
    let cfg = zapmomo::asr::config::resolve(asr_settings.as_ref(), None)?;

    // 预检模型文件，失败同步返回清晰错误（避免在后台线程里才报错）
    let files = [&cfg.encoder, &cfg.decoder, &cfg.joiner, &cfg.tokens];
    if let Some(missing) = files.iter().find(|p| !p.is_file()) {
        return Err(format!(
            "缺少模型文件: {}\n\n请在「配置」面板点击「下载模型」，或运行 `zapmomo asr install-model` 下载模型。",
            missing.display()
        ));
    }

    let running = state.running.clone();
    running.store(true, Ordering::Relaxed);
    // RuntimeActual：记录本次识别会话使用的模型目录；随线程退出自动清空
    let _active_guard = ActiveModelGuard::set(&state.active_model_dir, cfg.model_dir.clone());
    let thread_app = app.clone();
    let handle = std::thread::spawn(move || {
        let _active = _active_guard;
        tracing::info!("ASR listen thread started");
        let mut reaction = TauriAsrReaction { app: thread_app };
        let result = zapmomo::asr::run_realtime_with(
            &cfg,
            device.as_deref(),
            None,
            &mut reaction,
            Some(&running),
        );
        running.store(false, Ordering::Relaxed);
        match &result {
            Ok(()) => tracing::info!("ASR listen thread finished (clean)"),
            Err(e) => tracing::error!("ASR listen thread finished with error: {e}"),
        }
        let payload = ListenStopped {
            error: result.err(),
        };
        let _ = reaction.app.emit("asr-stopped", payload);
    });
    *state
        .handle
        .lock()
        .expect("asr listen handle lock poisoned") = Some(handle);
    // 通知前端识别已启动（含切换设备后的自动重启；启动瞬间前端未订阅时静默丢弃）
    let _ = app.emit("asr-started", ListenStopped { error: None });
    Ok(())
}

/// 开始实时语音识别。 —— Tauri command 外壳，签名与前端契约不变。
#[tauri::command]
fn start_asr_listen(
    app: AppHandle,
    state: State<'_, AsrListenState>,
    device: Option<String>,
) -> Result<(), String> {
    start_asr_listen_impl(app, state.inner(), device)
}

/// 停止实时语音识别的内部实现（command 与「切换设备重启」共用）。
fn stop_asr_listen_inner(state: &AsrListenState) -> Result<(), String> {
    if !state.is_listening() {
        return Err("当前没有在识别".to_string());
    }
    state.running.store(false, Ordering::Relaxed);
    let handle = state
        .handle
        .lock()
        .expect("asr listen handle lock poisoned")
        .take();
    if let Some(handle) = handle {
        let _ = handle.join();
    }
    *state
        .active_model_dir
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

/// 停止实时语音识别：置停止标志并等待线程退出。
#[tauri::command]
fn stop_asr_listen(state: State<'_, AsrListenState>) -> Result<(), String> {
    stop_asr_listen_inner(state.inner())
}

/// 当前是否正在识别。
#[tauri::command]
fn is_asr_listening(state: State<'_, AsrListenState>) -> bool {
    state.is_listening()
}

/// 下载并安装 ASR 模型（默认安装到 `~/.zapmomo/models/<模型名>`）。
///
/// 防重入；下载在阻塞线程池执行，进度经 `asr-model-download-progress` 事件推给前端。
#[tauri::command]
async fn download_asr_model(
    app: AppHandle,
    state: State<'_, AsrDownloadState>,
) -> Result<(), String> {
    let flag = state.in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("模型下载已在进行中，请稍候".to_string());
    }
    let dest = zapmomo::asr::user_model_dir();
    let punct_dest = zapmomo::asr::punctuation_user_model_dir();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ResetOnDrop(flag);
        let mut progress = |p: zapmomo::asr::DownloadProgress| {
            let stage = match p.stage {
                zapmomo::asr::DownloadStage::Downloading => "downloading",
                zapmomo::asr::DownloadStage::Verifying => "verifying",
                zapmomo::asr::DownloadStage::Extracting => "extracting",
                zapmomo::asr::DownloadStage::Done => "done",
            };
            let _ = app.emit(
                "asr-model-download-progress",
                DownloadProgressPayload {
                    stage: stage.to_string(),
                    percent: p.percent,
                    message: p.message,
                },
            );
        };
        zapmomo::asr::install_model_to(&dest, false, &mut progress).map_err(|e| e.to_string())?;
        // 顺带安装标点模型（自动开启）；失败仅警告，不阻断 ASR 下载成功。
        if let Err(e) =
            zapmomo::asr::install_punctuation_model_to(&punct_dest, false, &mut progress)
        {
            tracing::warn!("标点模型安装失败（ASR 仍可用，仅无标点）: {e}");
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("下载任务异常: {e}"))?
}

/// TTS 合成线程状态：共享 busy 标志 + 线程句柄。
struct TtsSynthesizeState {
    busy: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl TtsSynthesizeState {
    fn new() -> Self {
        Self {
            busy: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        }
    }

    fn is_synthesizing(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }
}

/// TTS 模型下载状态：防重入标志。
struct TtsDownloadState {
    in_progress: Arc<AtomicBool>,
}

impl Default for TtsDownloadState {
    fn default() -> Self {
        Self {
            in_progress: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// GUI 展示用的 TTS 配置信息。
#[derive(Serialize)]
struct TtsConfigInfo {
    model_dir: String,
    provider: String,
    num_threads: i32,
    enabled: bool,
    models_present: bool,
    model_downloading: bool,
    settings_path: String,
    /// 扩散解码步数（质量/速度权衡），可经 `set_tts_params` 修改
    num_steps: i32,
    /// 默认语速，可经 `set_tts_params` 修改
    speed: f32,
    /// 调试输出，可经 `set_tts_params` 修改
    debug: bool,
    /// 默认音色 id（`None` = 用内置 leijun），可经 `set_tts_voice` 修改
    voice: Option<String>,
}

/// 合成结果事件载荷（推给前端播放）。
#[derive(Clone, Serialize)]
struct TtsResult {
    path: String,
    duration: f32,
    sample_rate: i32,
}

/// 读取合并后的 TTS 配置（settings.toml + 默认值），并给出模型是否就绪。
#[tauri::command]
fn get_tts_config(state: State<'_, TtsDownloadState>) -> Result<TtsConfigInfo, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    let cfg = zapmomo::tts::config::resolve(tts_settings.as_ref(), None)?;

    let files = [
        &cfg.encoder,
        &cfg.decoder,
        &cfg.vocoder,
        &cfg.tokens,
        &cfg.lexicon,
    ];
    let models_present = files.iter().all(|p| p.is_file()) && cfg.data_dir.is_dir();

    Ok(TtsConfigInfo {
        model_dir: cfg.model_dir.display().to_string(),
        provider: cfg.provider.clone(),
        num_threads: cfg.num_threads,
        enabled: cfg.enabled,
        models_present,
        model_downloading: state.in_progress.load(Ordering::Relaxed),
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
        num_steps: cfg.num_steps,
        speed: cfg.speed,
        debug: cfg.debug,
        voice: cfg.voice.clone(),
    })
}

/// 在后台线程内合成文本，期间发 `tts-progress`，完成后发 `tts-result`。
fn synthesize_inner(
    app: &AppHandle,
    cfg: &zapmomo::tts::config::ResolvedTtsConfig,
    text: &str,
    speed: f32,
    reference_wav: &Path,
    reference_text: &str,
) -> Result<(), String> {
    let engine = zapmomo::tts::TtsEngine::new(cfg.clone())?;
    let out_dir = zapmomo::config::settings::get_tts_output_dir();
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("创建输出目录失败: {e}"))?;
    // 放行 asset 协议 scope，前端 <audio> 才能通过 asset:// 播放生成的 wav。
    let _ = app.asset_protocol_scope().allow_directory(&out_dir, true);
    let out_path = zapmomo::tts::default_output_path();

    let progress_app = app.clone();
    let sample_count = engine.synthesize_to_wav_with_progress(
        text,
        speed,
        reference_wav,
        reference_text,
        &out_path,
        move |p| {
            let _ = progress_app.emit(
                "tts-progress",
                zapmomo::tts::reaction::TtsProgress { percent: p },
            );
            true
        },
    )?;

    let sample_rate = engine.sample_rate();
    let duration = sample_count as f32 / sample_rate as f32;
    let _ = app.emit(
        "tts-result",
        TtsResult {
            path: out_path.display().to_string(),
            duration,
            sample_rate,
        },
    );
    Ok(())
}

/// 列出可用参考音色：模型包内置 + 用户自定义音色库。
#[tauri::command]
fn list_tts_voices() -> Result<Vec<zapmomo::tts::TtsVoice>, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    let cfg = zapmomo::tts::config::resolve(tts_settings.as_ref(), None)?;
    let mut voices = zapmomo::tts::voice::list_builtin_voices(&cfg.model_dir);
    voices.extend(zapmomo::tts::voice_store::list_custom_voices());
    Ok(voices)
}

/// 保存一个自定义音色：把源 wav 拷贝到音色库并登记（命名 + 参考转写文本）。
#[tauri::command]
fn save_tts_voice(
    name: String,
    source_wav_path: String,
    reference_text: String,
) -> Result<zapmomo::tts::TtsVoice, String> {
    zapmomo::tts::voice_store::save_voice(
        &name,
        std::path::Path::new(&source_wav_path),
        &reference_text,
    )
}

/// 删除一个自定义音色（清单 + wav 文件）。
#[tauri::command]
fn delete_tts_voice(id: String) -> Result<(), String> {
    zapmomo::tts::voice_store::delete_voice(&id)
}

/// 录制 N 秒麦克风并保存为 16k wav，返回 wav 路径（供后续保存为音色）。
#[tauri::command]
async fn record_tts_voice(seconds: u32, device: Option<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        zapmomo::audio::record_voice(seconds, device.as_deref()).map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| format!("录音任务异常: {e}"))?
}

/// 用 ASR 离线转写参考音频，返回带标点的转写文本（供自定义音色自动填充）。
///
/// 依赖 ASR 模型（含标点模型）已下载；转写在阻塞线程池执行，避免卡住 UI。
#[tauri::command]
async fn transcribe_reference_audio(wav_path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = zapmomo::config::settings::load_settings()?;
        let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
        let cfg = zapmomo::asr::config::resolve(asr_settings.as_ref(), None)?;
        zapmomo::asr::transcribe_wav(&cfg, Path::new(&wav_path))
    })
    .await
    .map_err(|e| format!("转写任务异常: {e}"))?
}

/// 把文本合成为语音并写入 wav（后台线程执行）。
///
/// 校验模型文件后启动独立线程合成，进度经 `tts-progress` 事件推给前端；
/// 完成后发 `tts-result`（含 wav 路径），线程末发 `tts-stopped`。
#[tauri::command]
fn synthesize_tts(
    app: AppHandle,
    state: State<'_, TtsSynthesizeState>,
    text: String,
    speed: Option<f32>,
    voice: Option<String>,
    reference_wav: Option<String>,
    reference_text: Option<String>,
) -> Result<(), String> {
    if state.is_synthesizing() {
        return Err("正在合成中".to_string());
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("文本不能为空".to_string());
    }

    let settings = zapmomo::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    let cfg = zapmomo::tts::config::resolve(tts_settings.as_ref(), None)?;

    // 启用门控：关闭时直接返回错误，前端据此禁用合成。
    if !cfg.enabled {
        return Err("语音合成已禁用，请在「模型与能力」中开启语音合成。".to_string());
    }

    // 预检模型文件，失败同步返回清晰错误（避免在后台线程里才报错）
    let files = [
        &cfg.encoder,
        &cfg.decoder,
        &cfg.vocoder,
        &cfg.tokens,
        &cfg.lexicon,
    ];
    if let Some(missing) = files.iter().find(|p| !p.is_file()) {
        return Err(format!(
            "缺少模型文件: {}\n\n请在「配置」面板点击「下载模型」，或运行 `zapmomo tts install-model` 下载模型。",
            missing.display()
        ));
    }
    if !cfg.data_dir.is_dir() {
        return Err(format!(
            "缺少数据目录: {}\n\n请在「配置」面板点击「下载模型」，或运行 `zapmomo tts install-model` 下载模型。",
            cfg.data_dir.display()
        ));
    }

    // 解析参考音色：自定义 wav > 内置音色 id > 配置默认（在后台线程外解析，尽早报错）。
    let custom_wav = reference_wav.map(std::path::PathBuf::from);
    let (ref_wav, ref_text) = zapmomo::tts::voice::resolve_reference(
        &cfg,
        voice.as_deref(),
        custom_wav.as_deref(),
        reference_text.as_deref(),
    )?;

    let speed = speed.unwrap_or(cfg.speed);

    let busy = state.busy.clone();
    busy.store(true, Ordering::Relaxed);
    let thread_app = app.clone();
    let handle = std::thread::spawn(move || {
        tracing::info!("TTS synthesize thread started");
        let result = synthesize_inner(&thread_app, &cfg, &text, speed, &ref_wav, &ref_text);
        busy.store(false, Ordering::Relaxed);
        match &result {
            Ok(()) => tracing::info!("TTS synthesize thread finished (clean)"),
            Err(e) => tracing::error!("TTS synthesize thread finished with error: {e}"),
        }
        let payload = ListenStopped {
            error: result.err(),
        };
        let _ = thread_app.emit("tts-stopped", payload);
    });
    *state.handle.lock().expect("tts handle lock poisoned") = Some(handle);
    Ok(())
}

/// 停止 TTS 合成/播放的内部实现（command 与全局快捷键打断共用）。
fn stop_tts_inner(state: &TtsSynthesizeState) -> Result<(), String> {
    if !state.is_synthesizing() {
        return Err("当前没有在合成".to_string());
    }
    state.busy.store(false, Ordering::Relaxed);
    let handle = state
        .handle
        .lock()
        .expect("tts handle lock poisoned")
        .take();
    if let Some(handle) = handle {
        let _ = handle.join();
    }
    Ok(())
}

/// 停止正在进行的合成（等待线程退出）。
#[tauri::command]
fn stop_tts(state: State<'_, TtsSynthesizeState>) -> Result<(), String> {
    stop_tts_inner(state.inner())
}

/// 当前是否正在合成。
#[tauri::command]
fn is_tts_synthesizing(state: State<'_, TtsSynthesizeState>) -> bool {
    state.is_synthesizing()
}

/// 下载并安装 TTS 模型（主包 + 声码器，默认 `~/.zapmomo/models/<模型名>`）。
///
/// 防重入；下载在阻塞线程池执行，进度经 `tts-model-download-progress` 事件推给前端。
#[tauri::command]
async fn download_tts_model(
    app: AppHandle,
    state: State<'_, TtsDownloadState>,
) -> Result<(), String> {
    let flag = state.in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("模型下载已在进行中，请稍候".to_string());
    }
    let dest = zapmomo::tts::user_model_dir();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ResetOnDrop(flag);
        let mut progress = |p: zapmomo::tts::DownloadProgress| {
            let stage = match p.stage {
                zapmomo::tts::DownloadStage::Downloading => "downloading",
                zapmomo::tts::DownloadStage::Verifying => "verifying",
                zapmomo::tts::DownloadStage::Extracting => "extracting",
                zapmomo::tts::DownloadStage::Done => "done",
            };
            let _ = app.emit(
                "tts-model-download-progress",
                DownloadProgressPayload {
                    stage: stage.to_string(),
                    percent: p.percent,
                    message: p.message,
                },
            );
        };
        zapmomo::tts::install_model_to(&dest, false, &mut progress).map_err(|e| e.to_string())?;
        zapmomo::tts::install_vocoder_to(&dest, false, &mut progress).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| format!("下载任务异常: {e}"))?
}

/// `download_llm_model` 的返回：最终模型文件路径 + 是否写入了 settings 配置。
#[derive(Serialize)]
struct LlmDownloadResult {
    model_path: String,
    applied: bool,
}

/// 下载 LLM 预设模型（registry id，如 "qwen3-0.6b-q4-k-m"），完成后按需写入 [llm].model_path。
///
/// 复用模型库安装链路（staging → sha256 → 原子 commit → managed 元数据），并与模型库下载
/// 共用 ModelLibraryState（全应用同时只允许一个 managed 下载）。进度经
/// `llm-model-download-progress` 事件推送。模型加载不在此做：由前端确认无 voice 会话后
/// 调 load_llm_model（load_llm_impl 无切换保护，voice 运行中替换引擎会造成双模型）。
#[tauri::command]
async fn download_llm_model(
    app: AppHandle,
    state: State<'_, ModelLibraryState>,
    id: String,
) -> Result<LlmDownloadResult, String> {
    let flag = state.in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("模型下载已在进行中，请稍候".to_string());
    }
    state.cancel.store(false, Ordering::SeqCst);
    *state.current_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.clone());

    let model =
        model_library::registry::model_by_id(&id).ok_or_else(|| format!("未知的模型：{id}"))?;
    // 早退需手动复位（guard 在 spawn_blocking 闭包内，覆盖不到这里）
    if model.model_type != LibModelType::Llm
        || model.download.is_none()
        || model.file_name.is_none()
    {
        flag.store(false, Ordering::SeqCst);
        *state.current_id.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return Err("该模型不是可下载的 LLM 预设".to_string());
    }

    let app = app.clone();
    let cancel = state.cancel.clone();
    let install_cancel = cancel.clone();
    let current_id = state.current_id.clone();
    let final_file = tauri::async_runtime::spawn_blocking(move || {
        let _guard = LibraryDownloadGuard {
            in_progress: flag,
            cancel,
            current_id,
        };
        let final_file = model_library::managed_install_dir(model)
            .join(model.file_name.as_deref().unwrap_or_default());
        // 幂等预检：最终文件已存在（此前经模型库下载过）→ 跳过下载。
        // install_managed_model 总是先下载到全新 staging 再 commit，不预检会对已存在模型重复全量下载。
        if final_file.is_file() {
            let _ = app.emit(
                "llm-model-download-progress",
                DownloadProgressPayload {
                    stage: "done".to_string(),
                    percent: 100.0,
                    message: "模型已就绪".to_string(),
                },
            );
            return Ok(final_file);
        }
        let mut progress = |p: zapmomo::kws::model::DownloadProgress| {
            let _ = app.emit(
                "llm-model-download-progress",
                DownloadProgressPayload {
                    stage: download_stage_str(p.stage).to_string(),
                    percent: p.percent,
                    message: p.message,
                },
            );
        };
        model_library::install_managed_model(model, &mut progress, Some(&install_cancel))
            .map(|_| final_file)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("下载任务异常: {e}"))??;

    // 条件写配置：当前已配置且文件存在（用户手动选择过）则不覆盖；
    // 未配置 / 配置了缺失路径（models_present=false 的两种情况）都写入新下载的模型。
    let applied = match llm_resolved_config() {
        Ok(cfg) if cfg.model_path.is_file() => false,
        Ok(_) => {
            model_library::set_selected_model(LibModelType::Llm, &final_file)?;
            true
        }
        // 配置读取失败：不动 settings，仅返回路径（前端只刷新展示，不自动加载）
        Err(_) => false,
    };
    Ok(LlmDownloadResult {
        model_path: final_file.display().to_string(),
        applied,
    })
}

/// 本地 LLM 引擎状态：懒创建的 worker 线程引擎。
struct LlmState {
    engine: Arc<Mutex<Option<Arc<LlmEngine>>>>,
    /// 模型切换进行中（防二次切换 / 防止切换期间删除涉及的模型）
    switch_in_progress: Arc<AtomicBool>,
    /// 正在切换的目标模型路径（用于 `RuntimeStatus::Switching` 精确匹配）
    switch_target_path: Arc<Mutex<Option<PathBuf>>>,
}

impl LlmState {
    fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(None)),
            switch_in_progress: Arc::new(AtomicBool::new(false)),
            switch_target_path: Arc::new(Mutex::new(None)),
        }
    }

    fn is_switching(&self) -> bool {
        self.switch_in_progress.load(Ordering::Relaxed)
    }

    fn switch_target(&self) -> Option<PathBuf> {
        self.switch_target_path.lock().ok().and_then(|g| g.clone())
    }

    /// 当前实际加载的模型路径（RuntimeActual）。
    fn loaded_model_path(&self) -> Option<PathBuf> {
        self.engine
            .lock()
            .ok()
            .and_then(|e| e.as_ref().and_then(|e| e.loaded_model_path()))
    }
}

/// 共享 LLM 引擎是否正在生成（voice / GUI 任一在生成）。切换/卸载保护据此判断：
/// 仅当 LLM 真正在工作时才阻止，空闲（待唤醒）时允许切换（voice 会从共享槽感知新引擎）。
fn llm_engine_is_generating(llm: &LlmState) -> bool {
    llm.engine
        .lock()
        .ok()
        .and_then(|e| e.as_ref().map(|e| e.is_generating()))
        .unwrap_or(false)
}

/// RAII：模型切换事务 guard，所有出口（成功/失败/回滚/早退/panic）都复位标志。
struct LlmSwitchGuard {
    in_progress: Arc<AtomicBool>,
    target: Arc<Mutex<Option<PathBuf>>>,
}

impl LlmSwitchGuard {
    fn begin(state: &LlmState, target: PathBuf) -> Result<Self, String> {
        if state.switch_in_progress.swap(true, Ordering::SeqCst) {
            return Err("模型切换正在进行中，请稍候".to_string());
        }
        *state
            .switch_target_path
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(target);
        Ok(Self {
            in_progress: state.switch_in_progress.clone(),
            target: state.switch_target_path.clone(),
        })
    }
}

impl Drop for LlmSwitchGuard {
    fn drop(&mut self) {
        self.in_progress.store(false, Ordering::SeqCst);
        *self.target.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// GUI 展示用的 LLM 配置信息。
#[derive(Serialize)]
struct LlmConfigInfo {
    enabled: bool,
    provider: String,
    model_path: String,
    models_present: bool,
    ready: bool,
    /// RuntimeActual：当前真正加载的模型路径（None = 未加载）
    loaded_model_path: Option<String>,
    enable_thinking: bool,
    auto_load: bool,
    settings_path: String,
    /// 当前生效的角色 system prompt
    system_prompt: String,
    /// 当前生效的采样/引擎参数（已 resolve，非 Option）
    params: GenParams,
}

/// 加载状态事件载荷。
#[derive(Clone, Serialize)]
struct LlmStatusPayload {
    ready: bool,
}

/// 读取合并后的 LLM 配置。
fn llm_resolved_config() -> Result<zapmomo::llm::config::ResolvedLlmConfig, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let llm_settings = settings.as_ref().and_then(|s| s.llm.clone());
    zapmomo::llm::config::resolve(llm_settings.as_ref(), None)
}

/// 把 LLM 引擎事件转发为 Tauri 事件，直到 `Finished`/`Error`（`stop_on_status` 时 `Status` 也终止）。
/// 持续把 LLM 引擎事件转发为 Tauri 事件，直到 `Error` / 引擎被释放（`Disconnected`；
/// `stop_on_status` 时 `Status` 也终止）。**`Finished` 不退出**——同一引擎每次生成
/// （GUI 对话 / voice 会话）都会继续产生 Token/Finished，单个转发线程持续服务，
/// 否则第二次生成的事件无人转发，前端会一直停在「生成中」。
/// 只持事件流 `rx`，不持引擎 Arc——引擎被替换后（set_current_model / load）无引用即
/// drop，旧 forward 线程随 `Disconnected` 退出，避免旧模型内存泄漏。
fn forward_llm_events(
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<LlmEvent>,
    stop_on_status: bool,
) {
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(LlmEvent::Token(delta)) => {
                let _ = app.emit("llm-token", delta);
            }
            Ok(LlmEvent::Finished(reason)) => {
                let _ = app.emit("llm-finished", reason);
                // 不 break：等待下一次生成的事件
            }
            Ok(LlmEvent::Error(e)) => {
                let _ = app.emit("llm-error", e);
                break;
            }
            Ok(LlmEvent::Status { ready }) => {
                let _ = app.emit("llm-status", LlmStatusPayload { ready });
                if stop_on_status {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // recv_timeout 本身已等待，无需额外 sleep
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// 读取 LLM 配置信息（模型路径 / 是否就绪 / 是否已下载）。
#[tauri::command]
fn get_llm_config(state: State<'_, LlmState>) -> Result<LlmConfigInfo, String> {
    let cfg = llm_resolved_config()?;
    let ready = state
        .engine
        .lock()
        .ok()
        .and_then(|e| e.as_ref().map(|e| e.is_ready()))
        .unwrap_or(false);
    let loaded_model_path = state
        .engine
        .lock()
        .ok()
        .and_then(|e| e.as_ref().and_then(|e| e.loaded_model_path()))
        .map(|p| p.display().to_string());
    Ok(LlmConfigInfo {
        enabled: cfg.enabled,
        provider: cfg.provider,
        model_path: cfg.model_path.display().to_string(),
        models_present: cfg.model_path.is_file(),
        ready,
        loaded_model_path,
        enable_thinking: cfg.params.enable_thinking,
        auto_load: cfg.auto_load,
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
        system_prompt: cfg.system_prompt,
        params: cfg.params,
    })
}

/// 加载 LLM 模型的核心逻辑（command 与启动自动加载共用）。
///
/// 加载在 worker 线程异步进行，结果经 `llm-status`/`llm-error` 事件返回。
fn load_llm_impl(app: AppHandle, state: &LlmState) -> Result<(), String> {
    // 统一引擎：加载会替换共享槽引擎，voice 空闲时可加载（voice 会感知新引擎）；
    // LLM 正在生成时禁止替换（避免破坏 voice / GUI 的当前生成）。
    if app.state::<VoiceSessionState>().is_running() && llm_engine_is_generating(state) {
        return Err("语音会话正在使用 LLM 生成回复，请稍候再加载。".to_string());
    }
    let cfg = llm_resolved_config()?;
    if !cfg.model_path.is_file() {
        return Err(format!(
            "模型文件不存在：{}\n\n请下载 GGUF 模型（如 Qwen3-4B-Instruct-2507 Q4_K_M）并在设置中配置路径。",
            cfg.model_path.display()
        ));
    }
    let engine = Arc::new(zapmomo::llm::LlmEngine::new(cfg).map_err(|e| e.to_string())?);
    engine.load().map_err(|e| e.to_string())?;
    *state.engine.lock().expect("llm lock poisoned") = Some(engine.clone());
    // 持续转发事件（voice 会话与 chat_llm 共用同一引擎，都由这一个 forward 转发，避免多线程重复 emit）
    std::thread::spawn(move || forward_llm_events(app, engine.subscribe(), false));
    Ok(())
}

/// 加载 LLM 模型（异步：结果经 `llm-status`/`llm-error` 事件返回）。
#[tauri::command]
fn load_llm_model(app: AppHandle, state: State<'_, LlmState>) -> Result<(), String> {
    // 统一引擎：voice 会话与 GUI 共用 LlmState 的 engine，load 幂等（已 ready 则无操作），无需拦截
    load_llm_impl(app, state.inner())
}

/// 卸载 LLM 模型并释放内存。仅当 LLM 正在生成时拒绝（语音会话空闲时可卸载，voice 会感知引擎变化）。
#[tauri::command]
fn unload_llm_model(app: AppHandle, state: State<'_, LlmState>) -> Result<(), String> {
    if app.state::<VoiceSessionState>().is_running() && llm_engine_is_generating(&state) {
        return Err("语音会话正在使用 LLM 生成回复，请稍候再卸载。".to_string());
    }
    let engine = state.engine.lock().expect("llm lock poisoned").take();
    if let Some(engine) = engine {
        engine.unload().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 发起一次流式对话（token 经 `llm-token`，结束经 `llm-finished`）。
///
/// 事件由统一的持续 forward（引擎就绪时 spawn）转发，此处不再额外 spawn。
#[tauri::command]
fn chat_llm(state: State<'_, LlmState>, text: String) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("文本不能为空".to_string());
    }
    let cfg = llm_resolved_config()?;
    let engine = state
        .engine
        .lock()
        .expect("llm lock poisoned")
        .clone()
        .ok_or("模型未加载，请先点击「加载模型」".to_string())?;
    if !engine.is_ready() {
        return Err("模型尚未就绪，请稍候".to_string());
    }
    let input = vec![InputItem::Message(ChatMessage::new(ChatRole::User, text))];
    engine
        .generate(input, cfg.params)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 取消当前生成。
#[tauri::command]
fn stop_llm(state: State<'_, LlmState>) -> Result<(), String> {
    if let Some(engine) = state.engine.lock().expect("llm lock poisoned").as_ref() {
        engine.cancel();
    }
    Ok(())
}

/// 模型是否已加载。
#[tauri::command]
fn is_llm_ready(state: State<'_, LlmState>) -> bool {
    state
        .engine
        .lock()
        .ok()
        .and_then(|e| e.as_ref().map(|e| e.is_ready()))
        .unwrap_or(false)
}

// ---- 语音会话（KWS→ASR→LLM→TTS 全链路）----

/// 把 `VoiceEvent` 转发为 Tauri 事件（`Started/BargeIn/Stopped` 是 CLI 噪音，忽略；
/// 终态由会话线程包装统一发 `voice-session-stopped`）。
fn make_voice_emit(app: AppHandle) -> Box<dyn Fn(VoiceEvent) + Send> {
    Box::new(move |ev| {
        // 镜像写入 tracing 日志（~/.zapmomo/logs/app.log），Tauri 模式下也能离线回溯语音会话
        zapmomo::voice::events::log_voice_event(&ev);
        match ev {
            VoiceEvent::Started
            | VoiceEvent::BargeIn
            | VoiceEvent::Stopped { .. }
            | VoiceEvent::FollowUp => {}
            VoiceEvent::State { state } => {
                let _ = app.emit(
                    "voice-session-state",
                    VoiceSessionStatePayload {
                        running: state != VoicePhase::Idle,
                        state,
                    },
                );
            }
            VoiceEvent::Wake { keyword } => {
                let _ = app.emit("voice-session-wake", VoiceWakePayload { keyword });
            }
            VoiceEvent::Transcript { text, is_final } => {
                // 最终用户句：持久化到对话记录（~/.zapmomo/conversations.json）
                if is_final {
                    records::append_record(records::ConversationRecord {
                        role: records::RecordRole::User,
                        text: text.clone(),
                        at: iso_timestamp_now(),
                    });
                }
                let _ = app.emit(
                    "voice-session-transcript",
                    VoiceTranscriptPayload { text, is_final },
                );
            }
            VoiceEvent::Token { delta } => {
                let _ = app.emit("voice-session-token", VoiceTokenPayload { delta });
            }
            VoiceEvent::ReplySentence { sentence } => {
                let _ = app.emit("voice-session-reply", VoiceReplyPayload { sentence });
            }
            VoiceEvent::PlaySentence { sentence } => {
                let _ = app.emit("voice-session-play", VoicePlayPayload { sentence });
            }
            VoiceEvent::ReplyFinished { reason, text } => {
                // 非空回复：持久化桌宠记录（空回复不落盘，避免空行）
                if let Some(text) = &text
                    && !text.is_empty()
                {
                    records::append_record(records::ConversationRecord {
                        role: records::RecordRole::Assistant,
                        text: text.clone(),
                        at: iso_timestamp_now(),
                    });
                }
                let _ = app.emit(
                    "voice-session-reply-finished",
                    VoiceReplyFinishedPayload { reason, text },
                );
            }
            VoiceEvent::Error { message, .. } => {
                let _ = app.emit("voice-session-error", VoiceErrorPayload { message });
            }
        }
    })
}

/// 启动语音会话：解析配置 → 确保 LlmState 唯一引擎存在 → spawn 会话线程
/// （线程内构造 `VoiceSession`，规避非 Send；注入共享 `Arc<LlmEngine>`，只加载一份模型）。
///
/// 会话构造/加载失败经 `voice-session-stopped{error}` 异步通知前端（启动静默降级）。
fn start_voice_session_impl(app: AppHandle, state: &VoiceSessionState) -> Result<(), String> {
    if state.is_running() {
        return Err("语音会话已在运行中".to_string());
    }
    let settings = zapmomo::config::settings::load_settings()?;
    let cfg = zapmomo::voice::config::resolve(settings.as_ref(), &VoiceCliOverrides::default())?;
    // 语音互动需 KWS 与 ASR 同时启用（持久化开关）：未启用则拒绝（自动/手动一致拦截）。
    let kws_enabled =
        zapmomo::kws::config::resolve(settings.as_ref().and_then(|s| s.kws.as_ref()), None)
            .map(|c| c.enabled)
            .unwrap_or(false);
    let asr_enabled =
        zapmomo::asr::config::resolve(settings.as_ref().and_then(|s| s.asr.as_ref()), None)
            .map(|c| c.enabled)
            .unwrap_or(false);
    if !(kws_enabled && asr_enabled) {
        return Err(
            "语音互动需要同时启用「唤醒词」(KWS) 与「语音识别」(ASR)。请在模型与能力页开启后重试。"
                .to_string(),
        );
    }
    // 同步预检模型文件：缺模型及时返回错误（也让 setup 的「voice 启动成功 → 跳过
    // LLM auto_load」判定可靠——voice 实际具备运行条件才返回 Ok）。
    preflight_voice_models(&cfg)?;

    // 统一 LLM 引擎：确保 `LlmState` 持有引擎（voice 与 GUI 共享，只加载一份）。
    // 未创建则创建并存入；加载延迟到 voice `run()` 内的 `load_blocking`。
    // voice 会话持「共享引擎槽」引用而非引擎 Arc 克隆——运行时引擎被外部切换
    // （set_current_model / load）时，voice 在每轮编排循环开头感知并重新绑定新引擎。
    let llm_state = app.state::<LlmState>();
    {
        let mut guard = llm_state.engine.lock().expect("llm lock poisoned");
        if guard.is_none() {
            let e =
                Arc::new(zapmomo::llm::LlmEngine::new(cfg.llm.clone()).map_err(|e| e.to_string())?);
            *guard = Some(e.clone());
            // 新引擎就绪后 spawn 持续 forward（GUI LLM 状态反映共享引擎）
            let app = app.clone();
            let e_for_fwd = e.clone();
            std::thread::spawn(move || forward_llm_events(app, e_for_fwd.subscribe(), false));
        }
    }

    let running = state.running.clone();
    running.store(true, Ordering::Relaxed);
    let emit = make_voice_emit(app.clone());
    let shared_llm_slot = llm_state.engine.clone();
    let handle = std::thread::spawn(move || {
        let mut session =
            match VoiceSession::new_with_parts(cfg, emit, running.clone(), Some(shared_llm_slot)) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("语音会话创建失败: {e}");
                    running.store(false, Ordering::Relaxed);
                    let _ = app.emit(
                        "voice-session-stopped",
                        VoiceStoppedPayload { error: Some(e) },
                    );
                    return;
                }
            };
        // 暴露打断标志给宿主（全局快捷键「打断播报」置位用）
        *app.state::<VoiceSessionState>()
            .barge_in
            .lock()
            .expect("voice barge_in lock poisoned") = Some(session.barge_in_flag());
        let result = session.run();
        running.store(false, Ordering::Relaxed);
        *app.state::<VoiceSessionState>()
            .barge_in
            .lock()
            .expect("voice barge_in lock poisoned") = None;
        match &result {
            Ok(()) => tracing::info!("语音会话结束"),
            Err(e) => tracing::error!("语音会话异常: {e}"),
        }
        let _ = app.emit(
            "voice-session-stopped",
            VoiceStoppedPayload {
                error: result.err(),
            },
        );
    });
    *state.handle.lock().expect("voice handle lock poisoned") = Some(handle);
    Ok(())
}

/// 预检语音会话所需模型文件（KWS / ASR / TTS / LLM）。缺任一返回带安装提示的错误。
fn preflight_voice_models(
    cfg: &zapmomo::voice::config::ResolvedSessionConfig,
) -> Result<(), String> {
    let files = [
        ("KWS encoder", &cfg.kws.encoder),
        ("KWS decoder", &cfg.kws.decoder),
        ("KWS joiner", &cfg.kws.joiner),
        ("KWS tokens", &cfg.kws.tokens),
        ("KWS keywords", &cfg.kws.keywords_file),
        ("ASR encoder", &cfg.asr.encoder),
        ("ASR decoder", &cfg.asr.decoder),
        ("ASR joiner", &cfg.asr.joiner),
        ("ASR tokens", &cfg.asr.tokens),
        ("TTS encoder", &cfg.tts.encoder),
        ("TTS decoder", &cfg.tts.decoder),
        ("TTS vocoder", &cfg.tts.vocoder),
        ("TTS tokens", &cfg.tts.tokens),
        ("TTS lexicon", &cfg.tts.lexicon),
    ];
    for (name, path) in files {
        if !path.is_file() {
            return Err(format!("缺少模型文件 {name}: {}", path.display()));
        }
    }
    if !cfg.tts.data_dir.is_dir() {
        return Err(format!("缺少 TTS 数据目录: {}", cfg.tts.data_dir.display()));
    }
    if !cfg.llm.model_path.is_file() {
        return Err(format!(
            "LLM 模型文件不存在: {}",
            cfg.llm.model_path.display()
        ));
    }
    Ok(())
}

/// 启动语音会话（进入待唤醒 Armed）。
#[tauri::command]
fn start_voice_session(app: AppHandle, state: State<'_, VoiceSessionState>) -> Result<(), String> {
    start_voice_session_impl(app, state.inner())
}

/// 停止语音会话的内部实现（command 与「切换设备重启」共用）。
fn stop_voice_session_inner(state: &VoiceSessionState) -> Result<(), String> {
    if !state.is_running() {
        return Err("语音会话未在运行中".to_string());
    }
    state.running.store(false, Ordering::Relaxed);
    if let Some(handle) = state
        .handle
        .lock()
        .expect("voice handle lock poisoned")
        .take()
    {
        let _ = handle.join();
    }
    // 会话线程 panic 时可能残留打断标志，这里兜底清空
    *state.barge_in.lock().expect("voice barge_in lock poisoned") = None;
    Ok(())
}

/// 停止语音会话（置停止标志并等待会话线程退出）。
#[tauri::command]
fn stop_voice_session(state: State<'_, VoiceSessionState>) -> Result<(), String> {
    stop_voice_session_inner(state.inner())
}

/// 语音会话是否在运行中。
#[tauri::command]
fn is_voice_session_running(state: State<'_, VoiceSessionState>) -> bool {
    state.is_running()
}

/// 读取持久化的对话记录（`~/.zapmomo/conversations.json`），供前端「对话记录」页载入。
#[tauri::command]
fn get_conversation_records() -> Vec<records::ConversationRecord> {
    records::load_records()
}

/// 清空持久化的对话记录。
#[tauri::command]
fn clear_conversation_records() -> Result<(), String> {
    records::clear_records()
}

/// 持久化用户选择的 LLM 模型路径（GGUF 文件），写入 `[llm].model_path`。
#[tauri::command]
fn set_llm_model_path(path: String) -> Result<(), String> {
    let path_buf = std::path::PathBuf::from(&path);
    if !path_buf.is_file() {
        return Err(format!("文件不存在：{path}"));
    }
    if !zapmomo::llm::local::llama::is_gguf_file(&path_buf) {
        return Err("不是有效的 GGUF 模型文件".to_string());
    }
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let llm = settings.llm.get_or_insert_with(LlmSettings::default);
    llm.model_path = Some(path);
    settings::save_settings(&settings)?;
    Ok(())
}

/// 持久化用户对 Qwen3 思考模式的开关，写入 `[llm].enable_thinking`。
#[tauri::command]
fn set_llm_thinking(enabled: bool) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let llm = settings.llm.get_or_insert_with(LlmSettings::default);
    llm.enable_thinking = Some(enabled);
    settings::save_settings(&settings)?;
    Ok(())
}

/// 持久化用户对「启动自动加载模型」的开关，写入 `[llm].auto_load`。
#[tauri::command]
fn set_llm_auto_load(enabled: bool) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let llm = settings.llm.get_or_insert_with(LlmSettings::default);
    llm.auto_load = Some(enabled);
    settings::save_settings(&settings)?;
    Ok(())
}

/// 批量持久化 LLM 采样/引擎参数（11 项），写入 `[llm]`。
///
/// 载荷为 `{ params: { context_size, temperature, ... } }`（snake_case 直传）；
/// `None` 字段保持原有配置不变。值先整体校验、再写入，出错时不部分修改。
#[tauri::command]
fn set_llm_params(params: LlmParamsPatch) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let llm = settings.llm.get_or_insert_with(LlmSettings::default);
    params.apply_to(llm)?;
    settings::save_settings(&settings)?;
    Ok(())
}

/// 持久化角色 system prompt，写入 `[llm].system_prompt`。
///
/// 空串会覆盖内置默认（模型收到空 system prompt）；改动需重新加载模型/provider 才生效。
#[tauri::command]
fn set_llm_system_prompt(prompt: String) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let llm = settings.llm.get_or_insert_with(LlmSettings::default);
    llm.system_prompt = Some(prompt);
    settings::save_settings(&settings)?;
    Ok(())
}

/// 持久化「是否启用语音合成」，写入 `[tts].enabled`（缺省 true）。
#[tauri::command]
fn set_tts_enabled(enabled: bool) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let tts = settings.tts.get_or_insert_with(TtsSettings::default);
    tts.enabled = Some(enabled);
    settings::save_settings(&settings)?;
    Ok(())
}

/// 批量持久化 TTS 合成参数（扩散步数/默认语速/线程/调试），写入 `[tts]`。
///
/// 载荷为 `{ params: { num_steps, speed, ... } }`（snake_case 直传）；
/// `None` 字段保持原有配置不变。值先整体校验、再写入，出错时不部分修改。
/// 引擎在每次合成时新建，因此保存后下一次合成即生效，无需重启。
#[tauri::command]
fn set_tts_params(params: TtsParamsPatch) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let tts = settings.tts.get_or_insert_with(TtsSettings::default);
    params.apply_to(tts)?;
    settings::save_settings(&settings)
}

/// 设定默认音色（写入 `[tts].voice`；`None` 恢复内置默认 leijun）。
///
/// 所有不显式指定音色的合成（测试语音 / 语音会话）都会用该默认音色，
/// 经 `resolve_reference` 回退生效。保存后下一次合成即生效，无需重启。
#[tauri::command]
fn set_tts_voice(voice: Option<String>) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let tts = settings.tts.get_or_insert_with(TtsSettings::default);
    tts.voice = voice;
    settings::save_settings(&settings)
}

/// 持久化「启用 KWS」开关，写入 `[kws].enabled`（缺省 false）。
/// 开关只持久化偏好；立即开始/停止监听由前端调用 `start_listen` / `stop_listen`，
/// 下次启动自动监听由 `.setup()` 判断 `[kws].enabled` 触发。
#[tauri::command]
fn set_kws_enabled(enabled: bool) -> Result<(), String> {
    tracing::info!("set_kws_enabled 命令被调用: enabled={enabled}");
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let kws = settings.kws.get_or_insert_with(KwsSettings::default);
    kws.enabled = Some(enabled);
    settings::save_settings(&settings)?;
    tracing::info!(
        "set_kws_enabled 已保存，[kws].enabled={:?}",
        settings.kws.as_ref().and_then(|k| k.enabled)
    );
    Ok(())
}

/// 持久化 ASR 启用状态，写入 `[asr].enabled`（语音会话「能识别」的前提）。
#[tauri::command]
fn set_asr_enabled(enabled: bool) -> Result<(), String> {
    tracing::info!("set_asr_enabled 命令被调用: enabled={enabled}");
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let asr = settings.asr.get_or_insert_with(AsrSettings::default);
    asr.enabled = Some(enabled);
    settings::save_settings(&settings)?;
    tracing::info!(
        "set_asr_enabled 已保存，[asr].enabled={:?}",
        settings.asr.as_ref().and_then(|a| a.enabled)
    );
    Ok(())
}

/// 持久化会话级自定义唤醒词，写入 `[kws].custom_keywords`（空串 → None = 模型内置）。
#[tauri::command]
fn set_kws_custom_keywords(keywords: String) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let kws = settings.kws.get_or_insert_with(KwsSettings::default);
    kws.custom_keywords = if keywords.trim().is_empty() {
        None
    } else {
        Some(keywords.trim().to_string())
    };
    settings::save_settings(&settings)
}

/// 持久化 KWS 引擎/运行参数（灵敏度/加权/块大小/线程/调试），写入 `[kws]`。
/// 引擎参数在启动监听时固化：修改后需重启监听才生效（由前端在保存后若在监听则重启）。
#[tauri::command]
fn set_kws_params(params: KwsParamsPatch) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let kws = settings.kws.get_or_insert_with(KwsSettings::default);
    params.apply_to(kws)?;
    settings::save_settings(&settings)
}

/// 持久化 ASR 引擎/运行参数（线程/块大小/断句/热词/标点/调试），写入 `[asr]`。
/// 引擎参数在启动识别时固化：修改后需重启识别才生效（由前端在保存后若在识别则重启）。
#[tauri::command]
fn set_asr_params(params: AsrParamsPatch) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let asr = settings.asr.get_or_insert_with(AsrSettings::default);
    params.apply_to(asr)?;
    settings::save_settings(&settings)
}

/// 读取全局默认麦克风输入设备名（空串 = 系统默认），KWS / ASR 共用。
#[tauri::command]
fn get_microphone() -> Result<String, String> {
    Ok(settings::load_settings()?
        .and_then(|s| s.microphone)
        .unwrap_or_default())
}

/// 设置并持久化全局默认麦克风（空串 → None = 系统默认）。
///
/// 若 KWS / ASR / 语音会话正在监听，用新设备自动重启对应监听，使切换立即生效；
/// 重启失败（如新设备不可用）返回错误，已停止的监听保持停止。
#[tauri::command]
fn set_microphone(
    app: AppHandle,
    listen: State<'_, ListenState>,
    asr_listen: State<'_, AsrListenState>,
    voice: State<'_, VoiceSessionState>,
    mic: String,
) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    settings.microphone = if mic.trim().is_empty() {
        None
    } else {
        Some(mic.trim().to_string())
    };
    settings::save_settings(&settings)?;

    let new_mic = settings.microphone.clone();

    // KWS 监听运行中 → 用新设备重启（custom_keywords 从持久化配置读取）。
    if listen.is_listening() {
        stop_listen_inner(listen.inner())?;
        let kw = settings
            .kws
            .as_ref()
            .and_then(|k| k.custom_keywords.clone());
        start_listen_impl(app.clone(), listen.inner(), new_mic.clone(), kw)?;
    }
    // ASR 监听运行中 → 用新设备重启。
    if asr_listen.is_listening() {
        stop_asr_listen_inner(asr_listen.inner())?;
        start_asr_listen_impl(app.clone(), asr_listen.inner(), new_mic.clone())?;
    }
    // 语音会话运行中 → 用新设备重启（会话内部 KWS/ASR 自持，重新加载新麦克风）。
    if voice.is_running() {
        stop_voice_session_inner(voice.inner())?;
        start_voice_session_impl(app.clone(), voice.inner())?;
    }
    Ok(())
}

/// GUI 展示用的 Live2D 配置信息。
#[derive(Serialize)]
struct Live2dConfigInfo {
    model_dir: Option<String>,
    model_file: Option<String>,
    format: Option<String>,
    models_present: bool,
    window_scale: Option<f64>,
    window_opacity: Option<f64>,
    settings_path: String,
}

/// 读取 Live2D 配置，并在模型目录存在时重新放行 asset 协议 scope。
///
/// asset 协议 scope 不跨进程持久，因此每次启动/读取都要重新
/// `allow_directory`，否则 WebView 无法加载模型文件。
#[tauri::command]
fn get_live2d_config(app: AppHandle) -> Result<Live2dConfigInfo, String> {
    let settings = settings::load_settings()?;
    let live2d_settings = settings.as_ref().and_then(|s| s.live2d.clone());
    let cfg = zapmomo::live2d::config::resolve(live2d_settings.as_ref())?;

    let models_present = cfg.model_file.as_ref().is_some_and(|f| f.is_file());
    if models_present {
        let _ = app
            .asset_protocol_scope()
            .allow_directory(&cfg.model_dir, true);
    }

    let window_scale = live2d_settings.as_ref().and_then(|l| l.window_scale);
    let window_opacity = live2d_settings.as_ref().and_then(|l| l.window_opacity);

    Ok(Live2dConfigInfo {
        model_dir: Some(cfg.model_dir.display().to_string()),
        model_file: cfg.model_file.map(|p| p.display().to_string()),
        format: cfg.format.map(|f| f.to_str().to_string()),
        models_present,
        window_scale,
        window_opacity,
        settings_path: settings::get_settings_path().display().to_string(),
    })
}

/// `live2d-model-changed` 事件载荷（切换到某伙伴 / 清屏）。
/// 字段为 `Option`：清屏时三字段均为 `None`。
#[derive(Clone, Serialize)]
struct Live2dModelInfo {
    model_dir: Option<String>,
    model_file: Option<String>,
    format: Option<String>,
}

// ---------------------------------------------------------------------------
// 伙伴库（Companion Library）命令
// ---------------------------------------------------------------------------

/// 前端展示用的伙伴信息（snake_case，与 `Live2dConfigInfo` 一致）。
#[derive(Serialize)]
struct CompanionView {
    id: String,
    name: String,
    source_path: Option<String>,
    model_dir: String,
    model_file: String,
    format: String,
    imported_at: String,
    /// 快速有效判定：托管目录与清单文件是否都还在磁盘上。
    valid: bool,
    /// 探测到的封面图绝对路径（best-effort；无封面图为 null，前端用占位图标）。
    cover_image: Option<String>,
}

#[derive(Serialize)]
struct CompanionLibraryView {
    models: Vec<CompanionView>,
    active_model_id: Option<String>,
}

#[derive(Serialize)]
struct ImportCompanionResult {
    library: CompanionLibraryView,
    model_id: String,
    already_imported: bool,
}

fn build_view(lib: &zapmomo::companion::CompanionLibrary) -> CompanionLibraryView {
    CompanionLibraryView {
        models: lib
            .models
            .iter()
            .map(|m| CompanionView {
                id: m.id.clone(),
                name: m.name.clone(),
                source_path: m.source_path.clone(),
                model_dir: m.model_dir.clone(),
                model_file: m.model_file.clone(),
                format: m.format.clone(),
                imported_at: m.imported_at.clone(),
                valid: zapmomo::companion::quick_valid(m),
                cover_image: zapmomo::live2d::config::find_cover_image(Path::new(&m.model_dir))
                    .map(|p| p.display().to_string()),
            })
            .collect(),
        active_model_id: lib.active_model_id.clone(),
    }
}

/// 把 `settings.toml [live2d].model_dir` 同步成伙伴库 active（**幂等**：值相同则
/// 不写不 emit，避免每次 `list_companions` 都触发桌宠重载）。
///
/// 唯一逻辑 Source of Truth 是 `CompanionLibrary.active_model_id`；settings 里的
/// `model_dir` 只是兼容 `CompanionRoot` / `get_live2d_config` / `live2d-model-changed`
/// 的 derived runtime cache，最终一致由本函数负责。
fn reconcile_active(
    app: &AppHandle,
    active: Option<&zapmomo::companion::CompanionModel>,
) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let live2d = settings.live2d.get_or_insert_with(Live2dSettings::default);

    let desired: Option<String> = active.map(|m| m.model_dir.clone());
    if live2d.model_dir == desired {
        return Ok(());
    }

    live2d.model_dir = desired;
    settings::save_settings(&settings)?;

    match active {
        Some(model) => {
            app.asset_protocol_scope()
                .allow_directory(Path::new(&model.model_dir), true)
                .map_err(|e| format!("无法放行模型目录: {e}"))?;
            let info = Live2dModelInfo {
                model_dir: Some(model.model_dir.clone()),
                model_file: Some(model.model_file.clone()),
                format: Some(model.format.clone()),
            };
            // 通知常驻角色窗口即时重载新模型（同进程事件，跨窗口同步）。
            let _ = app.emit("live2d-model-changed", &info);
        }
        None => {
            // 清屏：桌宠收到空 model_file 后清除当前模型。
            let info = Live2dModelInfo {
                model_dir: None,
                model_file: None,
                format: None,
            };
            let _ = app.emit("live2d-model-changed", &info);
        }
    }
    Ok(())
}

/// 启动阶段同步 reconcile（毫秒级，不迁移）：让 settings 与伙伴库 active 一致，
/// 使 `CompanionRoot` 挂载时 `get_live2d_config` 就读到正确的当前伙伴。
///
/// **库空时不主动清空 settings**：旧版 `settings.model_dir` 仍由后台迁移继续使用，
/// 避免「后台迁移完成前桌宠闪空」。只有库中解析出 active 时才应用。
fn reconcile_active_at_startup(app: &AppHandle) {
    match zapmomo::companion::load_library_fast() {
        Ok(lib) => {
            let active = zapmomo::companion::active_model(&lib);
            if let Some(model) = active
                && let Err(e) = reconcile_active(app, Some(model))
            {
                tracing::warn!("启动同步伙伴配置失败（将在下次打开伙伴页自愈）: {e}");
            }
        }
        Err(e) => tracing::warn!("读取伙伴库失败（跳过启动同步）: {e}"),
    }
}

/// 后台旧版迁移：库为空且旧 `[live2d].model_dir` 存在时，复制进托管目录并设为 active，
/// 完成后重新 reconcile（桌宠从旧目录无缝切到托管副本）。不阻塞启动。
fn migrate_legacy_in_background(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(
        move || match zapmomo::companion::migrate_legacy_if_empty() {
            Ok(Some(_id)) => {
                if let Ok(lib) = zapmomo::companion::load_library_fast() {
                    let active = zapmomo::companion::active_model(&lib);
                    if let Err(e) = reconcile_active(&app, active) {
                        tracing::warn!("迁移后同步伙伴配置失败: {e}");
                    }
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("旧版模型后台迁移失败（将在下次打开伙伴页重试）: {e}"),
        },
    );
}

/// 后台存量迁移：为已导入伙伴补注册未登记的动作/表情文件（幂等，不阻塞启动；
/// 失败不写标记，下次启动自动重试）。
fn register_motions_in_background() {
    tauri::async_runtime::spawn_blocking(move || {
        match zapmomo::companion::register_motions_for_existing() {
            Ok(n) if n > 0 => tracing::info!("已为 {n} 个伙伴补注册动作/表情文件"),
            Ok(_) => {}
            Err(e) => tracing::warn!("补注册动作/表情迁移失败（下次启动重试）: {e}"),
        }
    });
}

/// 列出伙伴库（含旧版迁移兜底 + sanitize active）。
#[tauri::command]
async fn list_companions(app: AppHandle) -> Result<CompanionLibraryView, String> {
    let lib = tauri::async_runtime::spawn_blocking(zapmomo::companion::load_library)
        .await
        .map_err(|e| e.to_string())??;
    // 放行所有有效托管目录的 asset scope（settings 窗口启动不再全局调 get_live2d_config，
    // 伙伴页预览依赖此处放行；scope 不跨进程持久，每次都要重新放行）。
    for model in &lib.models {
        if zapmomo::companion::quick_valid(model) {
            let _ = app
                .asset_protocol_scope()
                .allow_directory(Path::new(&model.model_dir), true);
        }
    }
    let active = zapmomo::companion::active_model(&lib);
    reconcile_active(&app, active)?;
    Ok(build_view(&lib))
}

/// 导入 Live2D 模型目录（复制到应用托管目录并登记进伙伴库）。
///
/// 成功或已导入都会立即放行新模型的 asset scope，保证右侧预览无需再进页面；
/// 若本次导入成为 active（首次导入自动 active）则 reconcile 同步桌宠。
#[tauri::command]
async fn import_companion(
    app: AppHandle,
    source_dir: String,
) -> Result<ImportCompanionResult, String> {
    let source = PathBuf::from(source_dir);
    let (model, already_imported) =
        tauri::async_runtime::spawn_blocking(move || zapmomo::companion::import_from_dir(&source))
            .await
            .map_err(|e| e.to_string())??;

    app.asset_protocol_scope()
        .allow_directory(Path::new(&model.model_dir), true)
        .map_err(|e| format!("无法放行模型目录: {e}"))?;

    let lib = zapmomo::companion::load_library_fast()?;
    let became_active = lib.active_model_id.as_deref() == Some(model.id.as_str());
    if became_active {
        let active = zapmomo::companion::active_model(&lib);
        reconcile_active(&app, active)?;
    }

    Ok(ImportCompanionResult {
        library: build_view(&lib),
        model_id: model.id.clone(),
        already_imported,
    })
}

/// 设置「当前使用」伙伴（Library 先持久化成功，再 reconcile 同步 settings 与桌宠）。
#[tauri::command]
async fn set_active_companion(app: AppHandle, id: String) -> Result<CompanionLibraryView, String> {
    let lib = tauri::async_runtime::spawn_blocking(move || zapmomo::companion::set_active(&id))
        .await
        .map_err(|e| e.to_string())??;
    let active = zapmomo::companion::active_model(&lib);
    reconcile_active(&app, active)?;
    Ok(build_view(&lib))
}

/// 重命名伙伴（只改展示名；不影响 active / 桌宠，reconcile 为幂等 no-op）。
#[tauri::command]
async fn rename_companion(
    app: AppHandle,
    id: String,
    name: String,
) -> Result<CompanionLibraryView, String> {
    let lib = tauri::async_runtime::spawn_blocking(move || zapmomo::companion::rename(&id, &name))
        .await
        .map_err(|e| e.to_string())??;
    let active = zapmomo::companion::active_model(&lib);
    reconcile_active(&app, active)?;
    Ok(build_view(&lib))
}

/// 移除伙伴：删除托管目录与库条目；若删的是 active，自动落到第一个有效伙伴或清空，
/// 并 reconcile 同步桌宠（切换到新 active 或清屏）。
#[tauri::command]
async fn remove_companion(app: AppHandle, id: String) -> Result<CompanionLibraryView, String> {
    let lib = tauri::async_runtime::spawn_blocking(move || zapmomo::companion::remove(&id))
        .await
        .map_err(|e| e.to_string())??;
    let active = zapmomo::companion::active_model(&lib);
    reconcile_active(&app, active)?;
    Ok(build_view(&lib))
}

/// 保存前端从 Live2D 渲染画布截取的 PNG 封面（写入托管目录 `cover.png`）。
#[tauri::command]
async fn save_cover_image(id: String, png: Vec<u8>) -> Result<CompanionLibraryView, String> {
    tauri::async_runtime::spawn_blocking(move || zapmomo::companion::save_cover(&id, &png))
        .await
        .map_err(|e| e.to_string())??;
    let lib = zapmomo::companion::load_library_fast()?;
    Ok(build_view(&lib))
}

/// 持久化角色窗口位置（逻辑像素），供下次启动恢复。
///
/// 由前端在用户手动拖动窗口后（debounce）调用，写入 `~/.zapmomo/settings.toml`
/// 的 `[live2d.window_position]` 段。
#[tauri::command]
fn save_companion_position(x: i32, y: i32) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let live2d = settings.live2d.get_or_insert_with(Live2dSettings::default);
    live2d.window_position = Some(CompanionWindowPosition { x, y });
    settings::save_settings(&settings)
}

/// 保存角色窗口缩放比例并通知角色窗口（内部实现，供 command 与原生菜单事件共用）。
fn apply_companion_scale(app: &AppHandle, scale: f64) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let live2d = settings.live2d.get_or_insert_with(Live2dSettings::default);
    live2d.window_scale = Some(scale);
    settings::save_settings(&settings)?;
    let _ = app.emit("companion-scale-changed", scale);
    rebuild_tray_menu(app);
    Ok(())
}

/// 保存角色窗口透明度并通知角色窗口（内部实现，供 command 与原生菜单事件共用）。
fn apply_companion_opacity(app: &AppHandle, opacity: f64) -> Result<(), String> {
    let opacity = clamp_opacity(opacity);
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let live2d = settings.live2d.get_or_insert_with(Live2dSettings::default);
    live2d.window_opacity = Some(opacity);
    settings::save_settings(&settings)?;
    let _ = app.emit("companion-opacity-changed", opacity);
    rebuild_tray_menu(app);
    Ok(())
}

/// 把原生菜单项 id 解析为缩放比例。
fn scale_from_id(id: &str) -> Option<f64> {
    match id {
        "scale_25" => Some(0.25),
        "scale_50" => Some(0.5),
        "scale_70" => Some(0.7),
        "scale_100" => Some(1.0),
        "scale_150" => Some(1.5),
        "scale_200" => Some(2.0),
        _ => None,
    }
}

/// 透明度合法范围（含边界）。
const OPACITY_MIN: f64 = 0.2;
const OPACITY_MAX: f64 = 1.0;

/// 把透明度 clamp 到 `[OPACITY_MIN, OPACITY_MAX]`。
fn clamp_opacity(v: f64) -> f64 {
    v.clamp(OPACITY_MIN, OPACITY_MAX)
}

/// 把原生菜单项 id 解析为透明度。
fn opacity_from_id(id: &str) -> Option<f64> {
    match id {
        "opacity_100" => Some(1.0),
        "opacity_80" => Some(0.8),
        "opacity_60" => Some(0.6),
        "opacity_40" => Some(0.4),
        "opacity_20" => Some(0.2),
        _ => None,
    }
}

/// 设置并持久化角色窗口缩放比例（1.0 = 100%）。
///
/// 由设置面板（或角色窗口自身）调用：写入 `~/.zapmomo/settings.toml` 的
/// `[live2d.window_scale]` 段，并通过 `companion-scale-changed` 事件通知角色窗口
/// （角色窗口持有真实模型宽高比，负责把比例换算成绝对尺寸并 `setSize`）。
#[tauri::command]
fn set_companion_scale(app: AppHandle, scale: f64) -> Result<(), String> {
    apply_companion_scale(&app, scale)
}

/// 设置并持久化角色窗口透明度（1.0 = 不透明，范围 0.2~1.0）。
///
/// 由设置面板调用：写入 `~/.zapmomo/settings.toml` 的 `[live2d].window_opacity`，
/// 并通过 `companion-opacity-changed` 事件通知角色窗口更新渲染层 opacity。
#[tauri::command]
fn set_companion_opacity(app: AppHandle, opacity: f64) -> Result<(), String> {
    apply_companion_opacity(&app, opacity)
}

/// 读取是否在 macOS Dock / Cmd+Tab 中隐藏应用图标（Accessory 模式）。
#[tauri::command]
fn get_hide_dock_icon() -> Result<bool, String> {
    Ok(settings::load_settings()?
        .unwrap_or_default()
        .hide_dock_icon)
}

/// 设置并持久化是否在 macOS Dock / Cmd+Tab 中隐藏应用图标，并立即生效。
///
/// 写入 `~/.zapmomo/settings.toml` 顶层的 `hide_dock_icon` 字段；非 macOS 仅持久化，
/// 不改变激活策略（该设置仅对 macOS 的 Dock / Cmd+Tab 有意义）。
///
/// `app` 仅在 macOS 上用于切换 ActivationPolicy，其它平台未使用，故非 macOS 允许未使用变量。
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
#[tauri::command]
fn set_hide_dock_icon(app: AppHandle, hide: bool) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    settings.hide_dock_icon = hide;
    settings::save_settings(&settings)?;
    #[cfg(target_os = "macos")]
    {
        let policy = if hide {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        };
        app.set_activation_policy(policy)
            .map_err(|e| format!("切换激活策略失败: {e}"))?;
    }
    Ok(())
}

/// 处理应用菜单、托盘菜单与角色窗口右键菜单事件。
fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "show_settings" | "open_settings" => show_settings_window(app),
        "toggle_companion" => toggle_companion_window(app),
        "hide_companion" => hide_companion_window(app),
        "restart" => app.request_restart(),
        "quit" => app.exit(0),
        _ => {
            if let Some(scale) = scale_from_id(id) {
                let _ = apply_companion_scale(app, scale);
            } else if let Some(opacity) = opacity_from_id(id) {
                let _ = apply_companion_opacity(app, opacity);
            }
        }
    }
}

/// 构建角色窗口的右键菜单（窗口尺寸/透明度子菜单 + 打开设置 / 隐藏角色 / 重启 / 退出）。
fn build_companion_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let (scale_submenu, opacity_submenu) = build_metric_submenus(app)?;
    let open_settings = MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide_companion", "隐藏角色", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "重启", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &scale_submenu,
            &opacity_submenu,
            &open_settings,
            &hide,
            &restart,
            &quit,
        ],
    )
}

/// 托盘 id（档位变化后 `tray_by_id` 定位托盘并重建菜单）。
const TRAY_ID: &str = "zapmomo-tray";

/// 构建托盘菜单：显示/隐藏角色、窗口尺寸/透明度、打开设置、重启、退出。
fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let (tray_scale, tray_opacity) = build_metric_submenus(app)?;
    let toggle_companion =
        MenuItem::with_id(app, "toggle_companion", "显示/隐藏角色", true, None::<&str>)?;
    let open_settings = MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "重启", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &toggle_companion,
            &tray_scale,
            &tray_opacity,
            &open_settings,
            &restart,
            &quit,
        ],
    )
}

/// 档位（尺寸/透明度）变化后重建托盘菜单，刷新勾选态。
///
/// 托盘菜单只在启动时构建一次，勾选态是当时的快照；不重建会出现旧档位残留打勾
/// （新档位被点击时自动勾上，快照里的旧档位没人取消）。
fn rebuild_tray_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID)
        && let Ok(menu) = build_tray_menu(app)
    {
        let _ = tray.set_menu(Some(menu));
    }
}

/// 读当前窗口缩放与透明度（读失败或缺省回退 1.0 / 1.0）。
fn current_companion_metrics() -> (f64, f64) {
    match settings::load_settings() {
        Ok(Some(s)) => {
            let live2d = s.live2d.as_ref();
            (
                live2d.and_then(|l| l.window_scale).unwrap_or(1.0),
                live2d.and_then(|l| l.window_opacity).unwrap_or(1.0),
            )
        }
        _ => (1.0, 1.0),
    }
}

/// 构建「窗口尺寸」「透明度」两个档位子菜单（角色右键菜单与托盘菜单共用）。
///
/// 档位用 `CheckMenuItem`：构建时读当前 settings，命中的档位打勾。
fn build_metric_submenus(
    app: &AppHandle,
) -> tauri::Result<(Submenu<tauri::Wry>, Submenu<tauri::Wry>)> {
    let (cur_scale, cur_opacity) = current_companion_metrics();
    let mk_item = |id: &str, label: &str, cur: f64, v: f64| {
        CheckMenuItem::with_id(app, id, label, true, v == cur, None::<&str>)
    };
    let s25 = mk_item("scale_25", "25%", cur_scale, 0.25)?;
    let s50 = mk_item("scale_50", "50%", cur_scale, 0.5)?;
    let s70 = mk_item("scale_70", "70%", cur_scale, 0.7)?;
    let s100 = mk_item("scale_100", "100%", cur_scale, 1.0)?;
    let s150 = mk_item("scale_150", "150%", cur_scale, 1.5)?;
    let s200 = mk_item("scale_200", "200%", cur_scale, 2.0)?;
    let o20 = mk_item("opacity_20", "20%", cur_opacity, 0.2)?;
    let o40 = mk_item("opacity_40", "40%", cur_opacity, 0.4)?;
    let o60 = mk_item("opacity_60", "60%", cur_opacity, 0.6)?;
    let o80 = mk_item("opacity_80", "80%", cur_opacity, 0.8)?;
    let o100 = mk_item("opacity_100", "100%", cur_opacity, 1.0)?;
    let scale_menu = Submenu::with_items(
        app,
        "窗口尺寸",
        true,
        &[&s25, &s50, &s70, &s100, &s150, &s200],
    )?;
    // 档位顺序与「窗口尺寸」一致：从小到大。
    let opacity_menu = Submenu::with_items(app, "透明度", true, &[&o20, &o40, &o60, &o80, &o100])?;
    Ok((scale_menu, opacity_menu))
}

/// 弹出角色窗口右键菜单（由前端在右键时调用，坐标相对窗口左上角，逻辑像素）。
#[tauri::command]
fn show_companion_menu(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    let menu = build_companion_menu(&app).map_err(|e| e.to_string())?;
    let window = app
        .get_webview_window("companion")
        .ok_or_else(|| "角色窗口不存在".to_string())?;
    window
        .popup_menu_at(&menu, LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

/// 显示设置窗口并聚焦。
fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 切换常驻角色窗口的显隐。
fn toggle_companion_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("companion") else {
        return;
    };
    if window.is_visible().unwrap_or(true) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 打开设置窗口（供角色窗口右键菜单调用）。
#[tauri::command]
fn open_settings(app: AppHandle) {
    show_settings_window(&app);
}

/// 打断当前回复：voice 会话运行中置位打断标志（会话线程停生成/合成/播放回 Armed）；
/// 同时兜底停独立 TTS 播放与 LLM 生成（voice 未运行但测试播放/生成中的场景）。
fn interrupt_reply(app: &AppHandle) {
    let voice = app.state::<VoiceSessionState>();
    if voice.is_running()
        && let Some(flag) = voice
            .barge_in
            .lock()
            .expect("voice barge_in lock poisoned")
            .clone()
    {
        flag.store(true, Ordering::Relaxed);
    }
    // 「没有在合成」不算错误：打断场景下静默跳过
    let _ = stop_tts_inner(app.state::<TtsSynthesizeState>().inner());
    if let Some(engine) = app
        .state::<LlmState>()
        .engine
        .lock()
        .expect("llm lock poisoned")
        .as_ref()
    {
        engine.cancel();
    }
}

/// 全局快捷键触发分发（复用托盘/菜单同款内部函数）。
fn dispatch_shortcut(app: &AppHandle, action: zapmomo::config::shortcuts::ShortcutAction) {
    use zapmomo::config::shortcuts::ShortcutAction;
    match action {
        ShortcutAction::ToggleCompanion => toggle_companion_window(app),
        ShortcutAction::OpenSettings => show_settings_window(app),
        ShortcutAction::InterruptReply => interrupt_reply(app),
        ShortcutAction::ToggleVoiceSession => {
            // stop 需 join 会话线程（等麦克风轮询退出）、start 有模型预检，
            // 都可能耗时：放后台线程避免阻塞快捷键回调
            let app = app.clone();
            std::thread::spawn(move || {
                let state = app.state::<VoiceSessionState>();
                let result = if state.is_running() {
                    stop_voice_session_inner(state.inner())
                } else {
                    start_voice_session_impl(app.clone(), state.inner())
                };
                if let Err(e) = result {
                    tracing::warn!("切换语音会话失败: {e}");
                }
            });
        }
    }
}

/// 启动时按 `[shortcuts]` 配置注册全局快捷键：单个失败仅告警不阻塞启动
/// （键位可能已被其他软件占用），其余照常注册。
fn register_shortcuts_at_startup(app: &AppHandle) {
    use zapmomo::config::shortcuts::ShortcutAction;
    let shortcuts = settings::load_settings()
        .ok()
        .flatten()
        .and_then(|s| s.shortcuts)
        .unwrap_or_default();
    for action in ShortcutAction::ALL {
        let Some(acc) = shortcuts.get(action).map(str::to_string) else {
            continue;
        };
        let result = app
            .global_shortcut()
            .on_shortcut(acc.as_str(), move |app, _sc, _ev| {
                dispatch_shortcut(app, action);
            });
        match result {
            Ok(()) => tracing::info!("全局快捷键已注册：{} = {}", action.as_str(), acc),
            Err(e) => tracing::warn!(
                "全局快捷键 {} ({}) 注册失败，已跳过: {e}",
                action.as_str(),
                acc
            ),
        }
    }
}

/// 读取用户自定义快捷键（action 标识 → accelerator，仅含已绑定项）。
#[tauri::command]
fn get_shortcuts() -> Result<std::collections::HashMap<String, String>, String> {
    let shortcuts = settings::load_settings()?
        .unwrap_or_default()
        .shortcuts
        .unwrap_or_default();
    let mut map = std::collections::HashMap::new();
    for action in zapmomo::config::shortcuts::ShortcutAction::ALL {
        if let Some(acc) = shortcuts.get(action) {
            map.insert(action.as_str().to_string(), acc.to_string());
        }
    }
    Ok(map)
}

/// 绑定快捷键：校验 → 查重 → **先注册成功再落盘**（键位被系统/其他应用占用时
/// 注册失败，配置保持原值，杜绝「界面已绑定但实际不生效」的假状态）。
#[tauri::command]
fn set_shortcut(app: AppHandle, action: String, accelerator: String) -> Result<(), String> {
    use zapmomo::config::shortcuts::{ShortcutAction, validate_accelerator};
    let action =
        ShortcutAction::from_ident(&action).ok_or_else(|| format!("未知的操作：{action}"))?;
    let accelerator = accelerator.trim().to_string();
    validate_accelerator(&accelerator)?;

    let mut cfg = settings::load_settings()?.unwrap_or_default();
    let shortcuts = cfg.shortcuts.get_or_insert_with(Default::default);
    if let Some(other) = shortcuts.find_conflict(action, &accelerator) {
        return Err(format!("该快捷键已绑定到「{}」", other.label()));
    }
    // 幂等：与当前值相同直接成功
    if shortcuts.get(action) == Some(accelerator.as_str()) {
        return Ok(());
    }
    let old = shortcuts.get(action).map(str::to_string);
    app.global_shortcut()
        .on_shortcut(accelerator.as_str(), move |app, _sc, _ev| {
            dispatch_shortcut(app, action);
        })
        .map_err(|e| format!("注册失败，可能已被其他应用占用：{e}"))?;
    // 新键注册成功后才解绑旧键
    if let Some(old) = old
        && let Err(e) = app.global_shortcut().unregister(old.as_str())
    {
        tracing::warn!("解绑旧快捷键 {old} 失败: {e}");
    }
    shortcuts.set(action, Some(accelerator));
    settings::save_settings(&cfg)?;
    Ok(())
}

/// 清除操作的快捷键绑定（解绑 + 配置置空）。
#[tauri::command]
fn clear_shortcut(app: AppHandle, action: String) -> Result<(), String> {
    use zapmomo::config::shortcuts::ShortcutAction;
    let action =
        ShortcutAction::from_ident(&action).ok_or_else(|| format!("未知的操作：{action}"))?;
    let mut cfg = settings::load_settings()?.unwrap_or_default();
    if let Some(shortcuts) = cfg.shortcuts.as_mut() {
        if let Some(cur) = shortcuts.get(action).map(str::to_string)
            && let Err(e) = app.global_shortcut().unregister(cur.as_str())
        {
            tracing::warn!("解绑快捷键 {cur} 失败: {e}");
        }
        shortcuts.set(action, None);
    }
    settings::save_settings(&cfg)?;
    Ok(())
}

/// 隐藏角色窗口。
fn hide_companion_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("companion") {
        let _ = window.hide();
    }
}

/// 隐藏角色窗口（供角色窗口右键菜单调用）。
#[tauri::command]
fn hide_companion(app: AppHandle) {
    hide_companion_window(&app);
}

/// 退出应用（供角色窗口右键菜单调用）。
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// 重启应用（退出后自动重新拉起，供设置页按钮调用）。
#[tauri::command]
fn restart_app(app: AppHandle) {
    app.request_restart();
}

// ===========================================================================
// 模型库（Model Library）
// ===========================================================================

/// 模型库下载任务状态：单任务 + 可取消 + 记录当前下载的模型 id。
#[derive(Default)]
struct ModelLibraryState {
    in_progress: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    current_id: Arc<Mutex<Option<String>>>,
}

/// 模型库下载进度事件载荷。
#[derive(Clone, Serialize)]
struct ModelLibraryProgressPayload {
    model_id: String,
    stage: String,
    asset: String,
    overall_percent: f64,
    bytes_downloaded: u64,
    total_bytes: u64,
    message: String,
}

/// 下载任务 guard：所有出口（成功/失败/取消/panic）都复位下载标志与 cancel。
struct LibraryDownloadGuard {
    in_progress: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    current_id: Arc<Mutex<Option<String>>>,
}

impl Drop for LibraryDownloadGuard {
    fn drop(&mut self) {
        self.in_progress.store(false, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);
        *self.current_id.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

fn download_stage_str(stage: zapmomo::kws::model::DownloadStage) -> &'static str {
    use zapmomo::kws::model::DownloadStage::*;
    match stage {
        Downloading => "downloading",
        Verifying => "verifying",
        Extracting => "extracting",
        Done => "done",
    }
}

/// 从模型库列表解析模型（按 `id` 或 `install_id`；Current/Delete 可唯一定位具体安装实例）。
fn resolve_library_model(id: &str) -> Result<LibraryModel, String> {
    model_library::resolve_model(id).ok_or_else(|| format!("未知的模型：{id}"))
}

/// 打开外部链接（仅供 "在 Hugging Face 查看"；只允许 http(s)）。
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("仅支持 http(s) 链接".to_string());
    }
    open_path(Path::new(&url))
}

/// 平台化打开目录（macOS `open` / Linux `xdg-open` / Windows `explorer`）。
fn open_path(p: &Path) -> Result<(), String> {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(cmd)
        .arg(p)
        .spawn()
        .map_err(|e| format!("打开目录失败：{e}"))?;
    Ok(())
}

/// 模型库列表（含每个模型的安装状态 / current / runtime_status）。
#[tauri::command]
fn list_model_library(
    kws: State<'_, ListenState>,
    asr: State<'_, AsrListenState>,
    llm: State<'_, LlmState>,
) -> Result<Vec<LibraryModel>, String> {
    let mut models = model_library::list_models();
    let kws_actual = kws.active_model_dir();
    let asr_actual = asr.active_model_dir();
    // 统一 LLM 引擎：voice 与 GUI 共用 LlmState 的 engine，状态直接反映其加载路径
    let llm_actual = llm.loaded_model_path();
    let llm_target = llm.switch_target();
    let llm_error_path = llm
        .engine
        .lock()
        .ok()
        .and_then(|e| e.as_ref().and_then(|e| e.last_load_error()))
        .map(|e| e.model_path);
    let actuals = model_library::RuntimeActuals {
        kws: kws_actual.as_deref(),
        asr: asr_actual.as_deref(),
        llm: llm_actual.as_deref(),
        llm_switching: llm.is_switching(),
        llm_switch_target: llm_target.as_deref(),
        llm_load_error_path: llm_error_path.as_deref(),
    };
    model_library::enrich_runtime_status(&mut models, &actuals);
    Ok(models)
}

/// 系统资源（独立命令，CPU 采样在阻塞线程执行）。
#[tauri::command]
async fn get_system_resources() -> Result<SystemResources, String> {
    tauri::async_runtime::spawn_blocking(model_library::sysinfo::get_system_resources)
        .await
        .map_err(|e| format!("资源检测失败：{e}"))
}

/// 下载并安装模型库中的 registry 模型（单任务，真实进度，可取消）。
#[tauri::command]
async fn download_library_model(
    app: AppHandle,
    state: State<'_, ModelLibraryState>,
    id: String,
) -> Result<(), String> {
    let flag = state.in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("已有模型下载进行中，请稍候".to_string());
    }
    state.cancel.store(false, Ordering::SeqCst);
    *state.current_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.clone());

    let model = model_library::registry::model_by_id(&id)
        .ok_or_else(|| format!("未知的 Registry 模型：{id}"))?;
    if model.download.is_none() {
        flag.store(false, Ordering::SeqCst);
        *state.current_id.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return Err("该模型没有内置下载源，请使用「导入 GGUF」".to_string());
    }

    let app = app.clone();
    let cancel = state.cancel.clone();
    let current_id = state.current_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = LibraryDownloadGuard {
            in_progress: flag,
            cancel: cancel.clone(),
            current_id,
        };
        let emit = |stage: &str, percent: f64, msg: &str| {
            let _ = app.emit(
                "model-library-download-progress",
                ModelLibraryProgressPayload {
                    model_id: id.clone(),
                    stage: stage.to_string(),
                    asset: String::new(),
                    overall_percent: percent,
                    bytes_downloaded: 0,
                    total_bytes: 0,
                    message: msg.to_string(),
                },
            );
        };
        emit("preparing", 0.0, "准备下载…");
        let mut progress = |p: zapmomo::kws::model::DownloadProgress| {
            let _ = app.emit(
                "model-library-download-progress",
                ModelLibraryProgressPayload {
                    model_id: id.clone(),
                    stage: download_stage_str(p.stage).to_string(),
                    asset: String::new(),
                    overall_percent: p.percent,
                    bytes_downloaded: p.bytes_downloaded,
                    total_bytes: p.total_bytes,
                    message: p.message,
                },
            );
        };
        let install_cancel = cancel.clone();
        let result =
            model_library::install_managed_model(model, &mut progress, Some(&*install_cancel));
        match result {
            Ok(_) => {
                emit("done", 100.0, "模型安装完成");
                Ok(())
            }
            Err(zapmomo::kws::model::ModelError::Cancelled) => {
                emit("cancelled", 0.0, "已取消下载");
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| format!("下载任务异常：{e}"))?
}

/// 取消当前下载。
#[tauri::command]
fn cancel_model_download(state: State<'_, ModelLibraryState>) -> Result<(), String> {
    if !state.in_progress.load(Ordering::Relaxed) {
        return Err("没有正在进行的下载".to_string());
    }
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// 设为当前模型（「使用」）。
///
/// 只写 `model_dir` / `model_path`，**绝不写 enabled / 自动启动能力**。
/// LLM 走完整事务：验证 → 写 selection → 卸载旧 → 加载新 → 失败回滚。
#[tauri::command]
async fn set_current_model(
    app: AppHandle,
    llm: State<'_, LlmState>,
    kws: State<'_, ListenState>,
    asr: State<'_, AsrListenState>,
    id: String,
) -> Result<SetCurrentResult, String> {
    let model = resolve_library_model(&id)?;
    if model.install_state != LibInstallState::Installed {
        return Err("该模型未安装或正在下载，无法设为当前模型".to_string());
    }
    let path = PathBuf::from(model.local_path.clone().ok_or("该模型没有可用路径")?);
    let mt = model.model_type;

    // ---- KWS / ASR / TTS：只写 selection，不触碰 enabled ----
    if mt != LibModelType::Llm {
        model_library::set_selected_model(mt, &path)?;
        let (action, effective, message) = match mt {
            LibModelType::Kws if kws.is_listening() => (
                LibRuntimeAction::RestartRequired,
                false,
                format!(
                    "已将 {} 设为 KWS 当前模型，将在下次启动监听时生效",
                    model.display_name
                ),
            ),
            LibModelType::Asr if asr.is_listening() => (
                LibRuntimeAction::RestartRequired,
                false,
                format!(
                    "已将 {} 设为 ASR 当前模型，将在下次启动识别时生效",
                    model.display_name
                ),
            ),
            _ => (
                LibRuntimeAction::None,
                true,
                format!("已将 {} 设为当前模型", model.display_name),
            ),
        };
        return Ok(SetCurrentResult {
            model_type: mt,
            model_id: model.id,
            path: path.display().to_string(),
            runtime_action: action,
            effective_immediately: effective,
            message,
        });
    }

    // ---- LLM 事务 ----
    if !path.is_file() {
        return Err("模型文件不存在".to_string());
    }
    if !zapmomo::llm::local::llama::is_gguf_file(&path) {
        return Err("不是有效的 GGUF 模型文件".to_string());
    }
    // 统一引擎：voice 会话共享引擎槽。仅当 LLM 正在生成时拒绝切换（会破坏 voice 回复）；
    // 空闲（待唤醒）时允许切换——voice 每轮从共享槽重新绑定，能感知新引擎。
    if app.state::<VoiceSessionState>().is_running() && llm_engine_is_generating(&llm) {
        return Err("语音会话正在使用 LLM 生成回复，请稍候再切换模型。".to_string());
    }
    let _guard = LlmSwitchGuard::begin(llm.inner(), path.clone())?;

    let old_path =
        model_library::selection_path(LibModelType::Llm).map(|p| p.display().to_string());
    let was_loaded = llm
        .engine
        .lock()
        .ok()
        .map(|e| e.as_ref().is_some())
        .unwrap_or(false);

    // 1. 写新 selection（短锁）
    model_library::set_selected_model(LibModelType::Llm, &path)?;
    if !was_loaded {
        return Ok(SetCurrentResult {
            model_type: mt,
            model_id: model.id,
            path: path.display().to_string(),
            runtime_action: LibRuntimeAction::None,
            effective_immediately: true,
            message: format!(
                "已将 {} 设为 LLM 当前模型，将在下次加载时生效",
                model.display_name
            ),
        });
    }

    // 2. 替换引擎（旧引擎 Drop 会 join worker 并卸载旧模型），随后才加载新模型
    let cfg = llm_resolved_config()?;
    let new_engine = Arc::new(zapmomo::llm::LlmEngine::new(cfg).map_err(|e| e.to_string())?);
    let old = llm
        .engine
        .lock()
        .expect("llm lock poisoned")
        .replace(new_engine.clone());
    drop(old);

    let loader = new_engine.clone();
    let load_result = tauri::async_runtime::spawn_blocking(move || {
        loader.load_blocking(std::time::Duration::from_secs(600))
    })
    .await
    .map_err(|e| format!("加载任务异常：{e}"))?;

    match load_result {
        Ok(()) => {
            std::thread::spawn(move || forward_llm_events(app, new_engine.subscribe(), false));
            Ok(SetCurrentResult {
                model_type: mt,
                model_id: model.id,
                path: path.display().to_string(),
                runtime_action: LibRuntimeAction::Reloaded,
                effective_immediately: true,
                message: format!("已将 {} 设为 LLM 当前模型", model.display_name),
            })
        }
        Err(new_err) => {
            tracing::warn!("切换 LLM 到新模型失败：{new_err}");
            // 3. 回滚：恢复 selection + 尽力重载旧模型
            model_library::restore_selected_model(LibModelType::Llm, old_path)?;
            let old_cfg = llm_resolved_config().map_err(|e| format!("恢复配置失败：{e}"))?;
            let old_engine =
                Arc::new(zapmomo::llm::LlmEngine::new(old_cfg).map_err(|e| e.to_string())?);
            let _prev = llm
                .engine
                .lock()
                .expect("llm lock poisoned")
                .replace(old_engine.clone());
            let old_loader = old_engine.clone();
            let old_result = tauri::async_runtime::spawn_blocking(move || {
                old_loader.load_blocking(std::time::Duration::from_secs(600))
            })
            .await
            .map_err(|e| format!("恢复加载任务异常：{e}"))?;
            match old_result {
                Ok(()) => {
                    std::thread::spawn(move || {
                        forward_llm_events(app, old_engine.subscribe(), false)
                    });
                    Ok(SetCurrentResult {
                        model_type: mt,
                        model_id: model.id,
                        path: path.display().to_string(),
                        runtime_action: LibRuntimeAction::ReloadFailedRolledBack,
                        effective_immediately: true,
                        message: "模型切换失败，已恢复之前的模型".to_string(),
                    })
                }
                Err(old_err) => {
                    tracing::warn!("恢复旧 LLM 模型也失败：{old_err}");
                    llm.engine.lock().expect("llm lock poisoned").take();
                    Ok(SetCurrentResult {
                        model_type: mt,
                        model_id: model.id,
                        path: path.display().to_string(),
                        runtime_action: LibRuntimeAction::ReloadFailedRollbackFailed,
                        effective_immediately: false,
                        message: "模型切换失败，原模型也未能重新加载，请手动重新加载模型"
                            .to_string(),
                    })
                }
            }
        }
    }
}

/// 删除模型：managed 删文件；external 只移除注册。后端全量安全检查。
#[tauri::command]
fn delete_model(
    dl: State<'_, ModelLibraryState>,
    llm: State<'_, LlmState>,
    kws: State<'_, ListenState>,
    asr: State<'_, AsrListenState>,
    id: String,
) -> Result<(), String> {
    let model = resolve_library_model(&id)?;
    let downloading = dl.in_progress.load(Ordering::Relaxed)
        && dl
            .current_id
            .lock()
            .map(|g| g.as_deref() == Some(id.as_str()))
            .unwrap_or(false);
    if downloading {
        return Err("该模型正在下载，请先取消下载".to_string());
    }
    if model.model_type == LibModelType::Llm && llm.is_switching() {
        return Err("模型切换正在进行中，请稍候".to_string());
    }
    if model.current {
        return Err("该模型当前正在使用，请先切换到其他模型".to_string());
    }
    if let Some(lp) = &model.local_path {
        let lp = Path::new(lp);
        let loaded = llm
            .loaded_model_path()
            .is_some_and(|p| model_library::paths_equal(&p, lp))
            || kws
                .active_model_dir()
                .is_some_and(|d| model_library::paths_equal(&d, lp))
            || asr
                .active_model_dir()
                .is_some_and(|d| model_library::paths_equal(&d, lp));
        if loaded {
            return Err("该模型当前仍在运行，请先停止或切换模型".to_string());
        }
    }

    if let Some(ext_id) = model_library::external_binding_to_remove(&id) {
        // external：只移除注册，绝不删原始文件
        model_library::remove_local_model_record(&ext_id)?;
        return Ok(());
    }
    // HF 安装：删除具体 artifact 目录（只删该 variant），并清理空父目录
    if model.source == model_library::ModelSource::Hf {
        if let Some(lp) = &model.local_path {
            let dir = model_library::runtime_to_install_dir(Path::new(lp));
            model_library::delete_hf_install_dir(&dir)?;
        }
        return Ok(());
    }
    let reg = model_library::registry::model_by_id(&id)
        .ok_or_else(|| format!("未知的 Registry 模型：{id}"))?;
    // 优先按 local_path（双根定位后的实际位置）推导目录；无 local_path（NotInstalled）
    // 再回退主根标准目录——旧根存量也能删到，而不是对着新根路径误判/漏删。
    let dir = model
        .local_path
        .map(|lp| model_library::runtime_to_install_dir(Path::new(&lp)))
        .filter(|d| d.exists())
        .unwrap_or_else(|| model_library::managed_install_dir(reg));
    if dir.exists() {
        model_library::delete_managed_dir(&dir)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 自定义数据目录（存储位置）
// ---------------------------------------------------------------------------

/// 存储迁移状态：防重入 + 取消标志。
#[derive(Default)]
struct StorageMigrateState {
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl StorageMigrateState {
    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

/// 迁移 guard：所有出口（成功/失败/取消/panic）复位 running 与 cancel。
struct StorageMigrateGuard {
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl Drop for StorageMigrateGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);
    }
}

/// 检查「设置/迁移数据目录」是否被占用（下载中 / 语音会话 / 监听 / 迁移中）。
///
/// 命中返回具体错误。`migrate_storage` 额外检查 LLM 已加载（mmap 持有文件句柄）。
#[allow(clippy::too_many_arguments)]
fn check_storage_busy(
    dl_kws: &DownloadState,
    dl_asr: &AsrDownloadState,
    dl_tts: &TtsDownloadState,
    lib_dl: &ModelLibraryState,
    dl_mgr: &DownloadManager,
    voice: &VoiceSessionState,
    kws: &ListenState,
    asr: &AsrListenState,
) -> Result<(), String> {
    if dl_kws.in_progress.load(Ordering::Relaxed)
        || dl_asr.in_progress.load(Ordering::Relaxed)
        || dl_tts.in_progress.load(Ordering::Relaxed)
        || lib_dl.in_progress.load(Ordering::Relaxed)
    {
        return Err("有模型正在下载，请先等待下载完成或取消后再操作".to_string());
    }
    if dl_mgr
        .snapshot()
        .iter()
        .any(|t| matches!(t.state.as_str(), "queued" | "downloading" | "verifying"))
    {
        return Err("有模型正在下载，请先等待下载完成或取消后再操作".to_string());
    }
    if voice.is_running() {
        return Err("语音会话正在运行，请先停止会话后再操作".to_string());
    }
    if kws.is_listening() || asr.is_listening() {
        return Err("有监听任务正在运行，请先停止后再操作".to_string());
    }
    Ok(())
}

/// 读取存储信息（当前/旧根、占用大小、迁移可用性、磁盘空间）。
#[tauri::command]
async fn get_storage_info(mig: State<'_, StorageMigrateState>) -> Result<StorageInfoView, String> {
    let mut info =
        tauri::async_runtime::spawn_blocking(zapmomo::model_library::storage::collect_storage_info)
            .await
            .map_err(|e| e.to_string())??;
    info.migrating = mig.is_running();
    Ok(info)
}

/// 设置（或清除）自定义数据目录。切换立即生效：新下载走新目录，存量模型保持可见可用。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn set_data_dir(
    app: AppHandle,
    path: Option<String>,
    dl_kws: State<'_, DownloadState>,
    dl_asr: State<'_, AsrDownloadState>,
    dl_tts: State<'_, TtsDownloadState>,
    lib_dl: State<'_, ModelLibraryState>,
    dl_mgr: State<'_, Arc<DownloadManager>>,
    voice: State<'_, VoiceSessionState>,
    kws: State<'_, ListenState>,
    asr: State<'_, AsrListenState>,
    mig: State<'_, StorageMigrateState>,
) -> Result<StorageInfoView, String> {
    if mig.is_running() {
        return Err("正在迁移模型，请稍候".to_string());
    }
    check_storage_busy(
        dl_kws.inner(),
        dl_asr.inner(),
        dl_tts.inner(),
        lib_dl.inner(),
        &dl_mgr,
        voice.inner(),
        kws.inner(),
        asr.inner(),
    )?;

    let data_dir_value = match &path {
        Some(p) if !p.trim().is_empty() => Some(
            zapmomo::model_library::storage::validate_data_dir(Path::new(p))?
                .display()
                .to_string(),
        ),
        _ => None,
    };
    zapmomo::model_library::update_settings(|cfg| {
        cfg.data_dir = data_dir_value.clone();
    })?;
    zapmomo::config::settings::refresh_data_dir_cache();
    let _ = app.emit("storage-dir-changed", ());

    tauri::async_runtime::spawn_blocking(zapmomo::model_library::storage::collect_storage_info)
        .await
        .map_err(|e| e.to_string())?
}

/// 迁移旧根存量到新数据目录（后台执行，进度经 `storage-migrate-progress` 事件推送）。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn migrate_storage(
    app: AppHandle,
    mig: State<'_, StorageMigrateState>,
    dl_kws: State<'_, DownloadState>,
    dl_asr: State<'_, AsrDownloadState>,
    dl_tts: State<'_, TtsDownloadState>,
    lib_dl: State<'_, ModelLibraryState>,
    dl_mgr: State<'_, Arc<DownloadManager>>,
    voice: State<'_, VoiceSessionState>,
    kws: State<'_, ListenState>,
    asr: State<'_, AsrListenState>,
    llm: State<'_, LlmState>,
) -> Result<(), String> {
    if mig.is_running() {
        return Err("迁移已在进行中".to_string());
    }
    check_storage_busy(
        dl_kws.inner(),
        dl_asr.inner(),
        dl_tts.inner(),
        lib_dl.inner(),
        &dl_mgr,
        voice.inner(),
        kws.inner(),
        asr.inner(),
    )?;
    if llm.loaded_model_path().is_some() {
        return Err("LLM 模型已加载（文件被占用），请先卸载模型后再迁移".to_string());
    }

    mig.running.store(true, Ordering::SeqCst);
    mig.cancel.store(false, Ordering::SeqCst);
    let running = mig.running.clone();
    let cancel = mig.cancel.clone();
    let emit_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = StorageMigrateGuard {
            running: running.clone(),
            cancel: cancel.clone(),
        };
        let outcome = zapmomo::model_library::storage::run_migration(
            false,
            &mut |p| {
                let _ = emit_app.emit("storage-migrate-progress", &p);
            },
            Some(&cancel),
        );
        match &outcome {
            Ok(o) => {
                if o.failed.is_empty() {
                    tracing::info!(
                        "存储迁移完成（moved={} skipped={}）",
                        o.moved.len(),
                        o.skipped.len()
                    );
                } else {
                    tracing::warn!("存储迁移部分失败：{:?}", o.failed);
                }
            }
            Err(e) => tracing::error!("存储迁移异常: {e}"),
        }
        outcome
    })
    .await
    .map_err(|e| format!("迁移任务异常: {e}"))??;

    // 迁移完成后：伙伴 active 已 relocate，重新 reconcile（allow_directory + 桌宠重载）
    if let Ok(lib) = zapmomo::companion::load_library_fast() {
        let active = zapmomo::companion::active_model(&lib);
        let _ = reconcile_active(&app, active);
        for model in &lib.models {
            if zapmomo::companion::quick_valid(model) {
                let _ = app
                    .asset_protocol_scope()
                    .allow_directory(Path::new(&model.model_dir), true);
            }
        }
    }
    let _ = app.emit("storage-dir-changed", ());
    Ok(())
}

/// 取消进行中的存储迁移（条目间/拷贝块间生效；已迁移条目保留）。
#[tauri::command]
fn cancel_storage_migration(mig: State<'_, StorageMigrateState>) -> Result<(), String> {
    if !mig.is_running() {
        return Err("当前没有迁移在运行".to_string());
    }
    mig.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// 在文件管理器中打开当前模型目录。
#[tauri::command]
fn open_storage_dir() -> Result<(), String> {
    open_path(&zapmomo::config::settings::get_models_dir())
}

/// 移除 external 模型注册（不删文件）。current / runtime-loaded / switching 时拒绝。
#[tauri::command]
fn remove_local_model(
    llm: State<'_, LlmState>,
    kws: State<'_, ListenState>,
    asr: State<'_, AsrListenState>,
    id: String,
) -> Result<(), String> {
    let rec = model_library::get_local_models()
        .into_iter()
        .find(|l| l.id == id)
        .ok_or("未找到该本地模型")?;
    let mt = LibModelType::from_str_value(&rec.model_type).unwrap_or(LibModelType::Llm);
    let lp = Path::new(&rec.path);
    if model_library::is_path_current(mt, lp) {
        return Err("该模型当前被设为当前模型，请先切换到其他模型".to_string());
    }
    if mt == LibModelType::Llm && llm.is_switching() {
        return Err("模型切换正在进行中，请稍候".to_string());
    }
    let running = llm
        .loaded_model_path()
        .is_some_and(|p| model_library::paths_equal(&p, lp))
        || kws
            .active_model_dir()
            .is_some_and(|d| model_library::paths_equal(&d, lp))
        || asr
            .active_model_dir()
            .is_some_and(|d| model_library::paths_equal(&d, lp));
    if running {
        return Err("该模型当前仍在运行，请先切换或卸载模型".to_string());
    }
    model_library::remove_local_model_record(&id)
}

/// 添加本地模型（Registry 卡片「导入 GGUF」显式携带 registry_id；顶部添加为 None）。
#[tauri::command]
fn add_local_model(
    llm: State<'_, LlmState>,
    path: String,
    model_type: Option<String>,
    registry_id: Option<String>,
) -> Result<LibraryModel, String> {
    // registry 重绑定时：旧绑定正在运行/切换中则拒绝
    if let Some(rid) = &registry_id
        && let Some(existing) = model_library::get_local_models()
            .into_iter()
            .find(|l| l.registry_id.as_deref() == Some(rid.as_str()))
    {
        let mt = LibModelType::from_str_value(&existing.model_type).unwrap_or(LibModelType::Llm);
        let ep = Path::new(&existing.path);
        if mt == LibModelType::Llm && llm.is_switching() {
            return Err("模型切换正在进行中，请稍候".to_string());
        }
        if llm
            .loaded_model_path()
            .is_some_and(|p| model_library::paths_equal(&p, ep))
        {
            return Err("该模型正在运行，请先切换或卸载后再重新导入".to_string());
        }
    }
    model_library::add_local_model(
        Path::new(&path),
        model_type.as_deref(),
        registry_id.as_deref(),
    )
}

/// 打开模型目录（后端按 id 解析真实路径，不接收任意 path）。
#[tauri::command]
fn open_model_directory(id: String) -> Result<(), String> {
    let model = resolve_library_model(&id)?;
    let path = model.local_path.ok_or("该模型没有安装路径")?;
    let p = PathBuf::from(&path);
    let dir = if p.is_dir() {
        p
    } else {
        p.parent().map(Path::to_path_buf).unwrap_or(p)
    };
    open_path(&dir)
}

// ===========================================================================
// 模型目录（Catalog）—— Provider-Neutral 在线目录
// ===========================================================================

/// 目录服务状态：持有 HF 客户端（缓存线程安全）。token/端点变更时整体重建。
struct CatalogState {
    client: Mutex<Arc<HfApiClient>>,
}

impl CatalogState {
    /// 从 settings 构建（base_url / token / 下载源）。
    fn from_settings() -> Self {
        let ml = zapmomo::config::settings::load_settings()
            .ok()
            .flatten()
            .and_then(|s| s.model_library)
            .unwrap_or_default();
        Self {
            client: Mutex::new(Arc::new(HfApiClient::from_settings(&ml))),
        }
    }

    /// 重建客户端（token / 端点变化后调用）。
    #[allow(dead_code)] // 由 Phase 3 的 catalog_set_token / catalog_set_endpoint 使用
    fn rebuild(&self) {
        let ml = zapmomo::config::settings::load_settings()
            .ok()
            .flatten()
            .and_then(|s| s.model_library)
            .unwrap_or_default();
        *self.client.lock().unwrap_or_else(|e| e.into_inner()) =
            Arc::new(HfApiClient::from_settings(&ml));
    }

    /// 取当前客户端引用（Arc clone，锁只保护 Arc 指针本身）。
    fn current(&self) -> Result<Arc<HfApiClient>, String> {
        self.client
            .lock()
            .map(|g| g.clone())
            .map_err(|e| format!("目录服务锁失效：{e}"))
    }
}

/// 解析 provider 参数（第一版仅支持 huggingface）。
fn require_hf_provider(provider: Option<&str>) -> Result<(), String> {
    match provider {
        None | Some("huggingface") => Ok(()),
        Some(other) => Err(format!("暂不支持的模型目录来源：{other}")),
    }
}

/// 搜索在线模型目录（分页）+ canonical merge（Verified 精选 + HF + 本地状态）。
/// `provider` 预留 ModelScope 等。
#[tauri::command]
async fn catalog_search_models(
    state: State<'_, CatalogState>,
    provider: Option<String>,
    query: CatalogQuery,
) -> Result<CatalogPage<zapmomo::model_library::catalog::UnifiedModelItem>, String> {
    use zapmomo::model_library::catalog::{curated_unified, merge_catalog};
    require_hf_provider(provider.as_deref())?;
    let client = state.current()?;
    let query_for_remote = query.clone();
    let remote = tauri::async_runtime::spawn_blocking(move || {
        client.search(&query_for_remote).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("目录请求异常：{e}"))??;
    let local_summary = model_library::local_install_summary();
    let curated = curated_unified(&query, &local_summary);
    Ok(merge_catalog(
        remote,
        curated,
        &local_summary,
        query.category,
    ))
}

/// 获取模型详情（仅元数据，不含完整文件树）。
#[tauri::command]
async fn catalog_get_model_detail(
    state: State<'_, CatalogState>,
    provider: Option<String>,
    model_id: String,
    revision: Option<String>,
) -> Result<RemoteModelDetail, String> {
    require_hf_provider(provider.as_deref())?;
    let client = state.current()?;
    tauri::async_runtime::spawn_blocking(move || {
        client
            .model_detail(&model_id, revision.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("目录请求异常：{e}"))?
}

/// 获取模型文件树（懒加载；Variant/Files/Compatibility 共用同一缓存）。
#[tauri::command]
async fn catalog_get_model_files(
    state: State<'_, CatalogState>,
    provider: Option<String>,
    model_id: String,
    revision: Option<String>,
) -> Result<Vec<zapmomo::model_library::catalog::RemoteModelFile>, String> {
    require_hf_provider(provider.as_deref())?;
    let client = state.current()?;
    tauri::async_runtime::spawn_blocking(move || {
        client
            .model_files(&model_id, revision.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("目录请求异常：{e}"))?
}

/// 兼容性判定（两阶段 Stage2：加载 files → ArchitectureDetector → Resolver → Artifacts）。
/// files 走共享缓存（Variant/Files/Compatibility 不重复请求）。
#[tauri::command]
async fn catalog_get_compatibility(
    state: State<'_, CatalogState>,
    provider: Option<String>,
    model_id: String,
    revision: Option<String>,
) -> Result<zapmomo::model_library::compat::Compatibility, String> {
    use zapmomo::model_library::compat::CompatibilityResolver;
    require_hf_provider(provider.as_deref())?;
    let client = state.current()?;
    tauri::async_runtime::spawn_blocking(move || {
        let files = client
            .model_files(&model_id, revision.as_deref())
            .map_err(|e| e.to_string())?;
        let compat = CompatibilityResolver::new().from_files(&model_id, &files);
        Ok(compat)
    })
    .await
    .map_err(|e| format!("目录请求异常：{e}"))?
}

/// 获取模型 README（懒加载）。
#[tauri::command]
async fn catalog_get_model_readme(
    state: State<'_, CatalogState>,
    provider: Option<String>,
    model_id: String,
    revision: Option<String>,
) -> Result<Option<String>, String> {
    require_hf_provider(provider.as_deref())?;
    let client = state.current()?;
    tauri::async_runtime::spawn_blocking(move || {
        client
            .model_readme(&model_id, revision.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("目录请求异常：{e}"))?
}

/// 当前下载配置（来自 settings；token 不经此结构传给前端）。
fn current_download_config() -> DownloadConfig {
    let ml = zapmomo::config::settings::load_settings()
        .ok()
        .flatten()
        .and_then(|s| s.model_library)
        .unwrap_or_default();
    DownloadConfig {
        catalog_base: ml.hf_catalog_base_url,
        download_source: ml.hf_download_source,
        mirror_url: ml.hf_mirror_url,
    }
}

/// 当前下载器（带 token；token 只进 Authorization header，不落日志）。
fn current_downloader() -> Arc<dyn zapmomo::model_library::download::FileDownloader> {
    let ml = zapmomo::config::settings::load_settings()
        .ok()
        .flatten()
        .and_then(|s| s.model_library)
        .unwrap_or_default();
    Arc::new(UreqFileDownloader::new(ml.hf_token, ml.hf_catalog_base_url))
}

/// 下载进度事件 sink：把任务视图推给前端（`download-progress`）。
struct TauriDownloadSink {
    app: AppHandle,
}

impl DownloadEventSink for TauriDownloadSink {
    fn on_update(&self, view: &DownloadTaskView) {
        let _ = self.app.emit("download-progress", view);
    }
}

/// 入队下载（顺序队列；独立 taskId，同 repo 多 variant 可并行排队）。
#[tauri::command]
fn download_enqueue(
    app: AppHandle,
    state: State<'_, Arc<DownloadManager>>,
    request: DownloadArtifactRequest,
) -> Result<DownloadTaskView, String> {
    let mgr = state.inner().clone();
    // 设置事件 sink（需要 AppHandle；幂等，每次覆盖）
    mgr.set_sink(Arc::new(TauriDownloadSink { app }));
    let cfg = current_download_config();
    mgr.enqueue(&request, &cfg)
}

/// 取消下载任务（Queued 直接移除；Downloading 置取消标志）。
#[tauri::command]
fn download_cancel(state: State<'_, Arc<DownloadManager>>, task_id: String) -> Result<(), String> {
    state.inner().cancel(&task_id)
}

/// 下载队列快照。
#[tauri::command]
fn download_snapshot(state: State<'_, Arc<DownloadManager>>) -> Vec<DownloadTaskView> {
    state.inner().snapshot()
}

/// 下载源视图（不含 token）。
#[tauri::command]
fn catalog_get_endpoint() -> EndpointConfigView {
    let ml = zapmomo::config::settings::load_settings()
        .ok()
        .flatten()
        .and_then(|s| s.model_library)
        .unwrap_or_default();
    EndpointConfigView {
        catalog_base: ml.hf_catalog_base_url,
        download_source: ml.hf_download_source,
        mirror_url: ml.hf_mirror_url,
    }
}

/// 设置下载源（写 settings + 重建客户端/下载器；token 不经此命令）。
#[tauri::command]
fn catalog_set_endpoint(
    state: State<'_, CatalogState>,
    dl: State<'_, Arc<DownloadManager>>,
    catalog_base: String,
    download_source: String,
    mirror_url: String,
) -> Result<(), String> {
    if !(download_source == "auto"
        || download_source == "huggingface"
        || download_source == "mirror"
        || download_source == "hf-mirror")
    {
        return Err("download_source 必须是 auto / huggingface / mirror".to_string());
    }
    if !(catalog_base.starts_with("https://") || catalog_base.starts_with("http://")) {
        return Err("catalog_base 必须是 http(s) 链接".to_string());
    }
    if !(mirror_url.is_empty()
        || mirror_url.starts_with("https://")
        || mirror_url.starts_with("http://"))
    {
        return Err("mirror_url 必须是 http(s) 链接".to_string());
    }
    let mirror_url = if mirror_url.trim().is_empty() {
        default_mirror_url()
    } else {
        mirror_url.trim().to_string()
    };
    model_library::update_settings(|cfg| {
        let lib = cfg.model_library.get_or_insert_with(Default::default);
        lib.hf_catalog_base_url = catalog_base;
        lib.hf_download_source = download_source;
        lib.hf_mirror_url = mirror_url;
    })?;
    state.rebuild();
    dl.inner().set_downloader(current_downloader());
    Ok(())
}

fn default_mirror_url() -> String {
    "https://hf-mirror.com".to_string()
}

/// 设置 Hugging Face token（明文 settings.toml；只进 Authorization header，不落日志/不出现在 View）。
#[tauri::command]
fn catalog_set_token(
    state: State<'_, CatalogState>,
    dl: State<'_, Arc<DownloadManager>>,
    token: Option<String>,
) -> Result<(), String> {
    model_library::update_settings(|cfg| {
        let lib = cfg.model_library.get_or_insert_with(Default::default);
        lib.hf_token = token;
    })?;
    state.rebuild();
    dl.inner().set_downloader(current_downloader());
    Ok(())
}

/// 下载源视图载荷（不含 token）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EndpointConfigView {
    catalog_base: String,
    download_source: String,
    mirror_url: String,
}

/// 角色窗口初始尺寸（逻辑像素，与 `setup` 中的 `inner_size` 保持一致）。
const COMPANION_INITIAL_W: f64 = 360.0;
const COMPANION_INITIAL_H: f64 = 480.0;
/// 角色窗口距屏幕工作区边缘的留白（逻辑像素）。
const COMPANION_MARGIN: f64 = 16.0;

/// 计算角色窗口首次出现的右下角位置（逻辑像素）。
///
/// 基于主屏 `work_area`（排除 Dock / 任务栏），把物理像素坐标除以 scale_factor
/// 转为逻辑像素，再减去窗口尺寸与留白得到窗口左上角坐标。
fn default_bottom_right_position(app: &AppHandle) -> Option<(f64, f64)> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let right = (work.position.x as f64 + work.size.width as f64) / scale;
    let bottom = (work.position.y as f64 + work.size.height as f64) / scale;
    Some((
        right - COMPANION_INITIAL_W - COMPANION_MARGIN,
        bottom - COMPANION_INITIAL_H - COMPANION_MARGIN,
    ))
}

/// Tauri 应用入口。
pub fn run() {
    zapmomo::logging::init_logging();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(ListenState::new())
        .manage(DownloadState::default())
        .manage(AsrListenState::new())
        .manage(AsrDownloadState::default())
        .manage(TtsSynthesizeState::new())
        .manage(TtsDownloadState::default())
        .manage(LlmState::new())
        .manage(VoiceSessionState::new())
        .manage(ModelLibraryState::default())
        .manage(CatalogState::from_settings())
        .manage(Arc::new(DownloadManager::new(current_downloader())))
        .manage(StorageMigrateState::default())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            list_devices,
            request_mic_permission,
            get_kws_config,
            set_kws_enabled,
            set_kws_custom_keywords,
            set_kws_params,
            start_listen,
            stop_listen,
            is_listening,
            download_kws_model,
            get_microphone,
            set_microphone,
            get_asr_config,
            set_asr_enabled,
            set_asr_params,
            start_asr_listen,
            stop_asr_listen,
            is_asr_listening,
            download_asr_model,
            get_tts_config,
            list_tts_voices,
            save_tts_voice,
            delete_tts_voice,
            record_tts_voice,
            transcribe_reference_audio,
            synthesize_tts,
            stop_tts,
            is_tts_synthesizing,
            download_tts_model,
            download_llm_model,
            get_llm_config,
            load_llm_model,
            unload_llm_model,
            chat_llm,
            stop_llm,
            is_llm_ready,
            set_llm_model_path,
            set_llm_thinking,
            set_llm_auto_load,
            set_llm_params,
            set_llm_system_prompt,
            set_tts_enabled,
            set_tts_params,
            set_tts_voice,
            start_voice_session,
            stop_voice_session,
            is_voice_session_running,
            get_conversation_records,
            clear_conversation_records,
            list_model_library,
            get_system_resources,
            download_library_model,
            cancel_model_download,
            set_current_model,
            delete_model,
            remove_local_model,
            add_local_model,
            open_model_directory,
            catalog_search_models,
            catalog_get_model_detail,
            catalog_get_model_files,
            catalog_get_compatibility,
            catalog_get_model_readme,
            open_external,
            download_enqueue,
            download_cancel,
            download_snapshot,
            catalog_get_endpoint,
            catalog_set_endpoint,
            catalog_set_token,
            get_storage_info,
            set_data_dir,
            migrate_storage,
            cancel_storage_migration,
            open_storage_dir,
            get_live2d_config,
            list_companions,
            import_companion,
            set_active_companion,
            rename_companion,
            remove_companion,
            save_cover_image,
            save_companion_position,
            set_companion_scale,
            set_companion_opacity,
            show_companion_menu,
            get_hide_dock_icon,
            set_hide_dock_icon,
            open_settings,
            get_shortcuts,
            set_shortcut,
            clear_shortcut,
            hide_companion,
            quit_app,
            restart_app
        ])
        .setup(|app| {
            // macOS：默认以普通应用出现（Dock + Cmd+Tab 可见，有全局菜单栏）；
            // 用户可在设置中开启「隐藏应用图标」，此时切换为 Accessory（从 Dock 与 Cmd+Tab 消失）。
            let loaded = settings::load_settings().ok().flatten();
            #[cfg(target_os = "macos")]
            let hide_dock_icon = loaded.as_ref().map(|s| s.hide_dock_icon).unwrap_or(false);

            #[cfg(target_os = "macos")]
            {
                app.handle().set_activation_policy(if hide_dock_icon {
                    tauri::ActivationPolicy::Accessory
                } else {
                    tauri::ActivationPolicy::Regular
                })?;
            }

            // 启动自动启动语音会话（若用户启用 voice）：进入待唤醒（Armed），失败静默降级。
            // voice 会话内部自带 KWS 与 LLM（自持引擎），因此自动启动成功时**跳过**
            // 下方独立的 LLM auto_load 与 KWS 自动监听——避免同一模型文件/麦克风设备
            // 被两份并发占用（llama.cpp 双 engine 并发加载会崩，cpal 同设备双路采集冲突）。
            let voice_auto_started = if loaded
                .as_ref()
                .and_then(|s| s.voice.as_ref())
                .and_then(|v| v.enabled)
                .unwrap_or(true)
            {
                let handle = app.handle().clone();
                let state = app.state::<VoiceSessionState>();
                match start_voice_session_impl(handle, state.inner()) {
                    Ok(()) => true,
                    Err(e) => {
                        tracing::warn!("自动启动语音会话失败: {e}");
                        false
                    }
                }
            } else {
                false
            };

            // 启动自动加载 LLM 模型（voice 未接管时，若用户开启 auto_load）：后台异步加载，
            // 失败静默降级为手动加载。
            if !voice_auto_started && llm_resolved_config().map(|c| c.auto_load).unwrap_or(false) {
                let handle = app.handle().clone();
                let state = app.state::<LlmState>();
                if let Err(e) = load_llm_impl(handle, state.inner()) {
                    tracing::warn!("自动加载 LLM 失败: {e}");
                }
            }

            // 启动自动监听 KWS（若用户启用 KWS 且未由语音会话代管）：后台线程监听，失败静默降级。
            // 使用持久化的麦克风（顶层 microphone）与自定义唤醒词（[kws].custom_keywords，空则模型内置）。
            if !voice_auto_started
                && zapmomo::kws::config::resolve(loaded.as_ref().and_then(|s| s.kws.as_ref()), None)
                    .map(|c| c.enabled)
                    .unwrap_or(false)
            {
                let handle = app.handle().clone();
                let state = app.state::<ListenState>();
                let mic = loaded.as_ref().and_then(|s| s.microphone.clone());
                let kw = loaded
                    .as_ref()
                    .and_then(|s| s.kws.as_ref())
                    .and_then(|k| k.custom_keywords.clone());
                if let Err(e) = start_listen_impl(handle, state.inner(), mic, kw) {
                    tracing::warn!("自动监听 KWS 失败: {e}");
                }
            }

            // 常驻角色窗口：透明、无边框、永远置顶、不入任务栏，静态展示 Live2D。
            // 复用顶部读到的 settings：同时恢复记忆的尺寸与位置。
            let live2d = loaded.as_ref().and_then(|s| s.live2d.clone());
            let scale = live2d.as_ref().and_then(|l| l.window_scale).unwrap_or(1.0);

            // 基准高度：min(480, 主屏工作区高度 × 0.6)。setup 阶段按默认 3:4 宽高比建窗，
            // 模型加载后前端按真实宽高比修正。
            let avail_height = app
                .primary_monitor()
                .ok()
                .flatten()
                .map(|m| {
                    let work = m.work_area();
                    (work.position.y as f64 + work.size.height as f64) / m.scale_factor()
                })
                .unwrap_or(1080.0);
            let init_h = 480.0_f64.min(avail_height * 0.6) * scale;
            let init_w = init_h * (3.0 / 4.0);

            // 启动同步 reconcile：让 settings 的 [live2d].model_dir 与伙伴库 active 一致，
            // 使 CompanionRoot 挂载时 get_live2d_config 直接读到正确的当前伙伴（毫秒级，不迁移）。
            reconcile_active_at_startup(app.handle());

            let mut companion = WebviewWindowBuilder::new(
                app,
                "companion",
                WebviewUrl::App("companion.html".into()),
            )
            .title("ZapMomo")
            .inner_size(init_w, init_h)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false);

            // macOS：首次点击直达 webview，可立即触发 startDragging（无需先点一下聚焦）。
            #[cfg(target_os = "macos")]
            {
                companion = companion.accept_first_mouse(true);
            }

            // 有记忆位置 → 恢复；否则 → 首次定位到屏幕右下角。
            if let Some(pos) = live2d.as_ref().and_then(|l| l.window_position.clone()) {
                companion = companion.position(pos.x as f64, pos.y as f64);
            } else if let Some((x, y)) = default_bottom_right_position(app.handle()) {
                companion = companion.position(x, y);
            }
            companion.build()?;

            // macOS：把角色窗口转成非激活面板，点击/拖动不抢前台焦点。
            #[cfg(target_os = "macos")]
            {
                use tauri_nspanel::{CollectionBehavior, StyleMask, WebviewWindowExt};

                let _ = app.handle().plugin(tauri_nspanel::init());
                if let Some(window) = app.get_webview_window("companion")
                    && let Ok(panel) = window.to_panel::<CompanionPanel>()
                {
                    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
                    panel.set_collection_behavior(
                        CollectionBehavior::new()
                            .stationary()
                            .move_to_active_space()
                            .full_screen_auxiliary()
                            .into(),
                    );
                }
            }

            // 后台旧版迁移：库为空且旧 [live2d].model_dir 存在时，把模型复制进托管目录并
            // 设为 active（完成后 reconcile，桌宠从旧目录无缝切到托管副本）。不阻塞启动。
            migrate_legacy_in_background(app.handle().clone());
            // 后台存量迁移：为已导入伙伴补注册未登记的动作/表情文件（幂等，不阻塞启动）。
            register_motions_in_background();

            // 设置窗口：默认隐藏，由 cmd+, 或托盘菜单打开；关闭时隐藏而非退出。
            let mut settings =
                WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
                    .title("ZapMomo 设置")
                    .inner_size(1180.0, 760.0)
                    .min_inner_size(1180.0, 640.0)
                    .resizable(true)
                    .visible(false);

            // macOS 用 titleBarStyle: Overlay 保留红绿灯；其它平台去掉系统标题栏。
            // title_bar_style / hidden_title 是 macOS 专属方法（Linux 上不存在），
            // 必须用 #[cfg] 编译期隔离，而非 cfg! 运行时判断。
            #[cfg(target_os = "macos")]
            {
                // macOS 保留系统红绿灯与阴影；窗口默认不透明。
                settings = settings
                    .title_bar_style(TitleBarStyle::Overlay)
                    .hidden_title(true)
                    .shadow(true);
            }
            // Linux：去掉系统标题栏，保留透明窗口供 CSS 圆角裁出（三键悬浮右上角，与 Windows 一致）。
            #[cfg(target_os = "linux")]
            {
                settings = settings.decorations(false).transparent(true);
            }
            // Windows：去掉系统标题栏即可；无 CSS 圆角处理，无需透明窗口
            //（不透明窗口性能更好）。同时关 DWM shadow：undecorated+shadow 会被
            // tao 在 WM_NCCALCSIZE 里左右底三边缩进客户区、由 DWM 画黑色窗框，
            // 而顶部 inset 在 Win10 强制为 0（否则画出原生标题栏），形成三边黑框；
            // 四边完整边框改由前端 AppShell 用 CSS 自绘。三键悬浮右上角。
            #[cfg(target_os = "windows")]
            {
                settings = settings.decorations(false).shadow(false);
            }
            settings.build()?;

            // 自动打开设置窗口：仅用于「无全局菜单栏」的场景（macOS Accessory 模式或非 macOS），
            // 否则 Cmd+, 快捷键不可靠，自动打开可避免「找不到设置」；普通模式有菜单栏，无需自动弹出。
            #[cfg(target_os = "macos")]
            let auto_open_settings = !hide_dock_icon;
            #[cfg(not(target_os = "macos"))]
            let auto_open_settings = true;
            if auto_open_settings {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    show_settings_window(&app_handle);
                });
            }

            // 应用菜单（仅 macOS）：「ZapMomo」子菜单（偏好设置 cmd+, / 退出 Cmd+Q）
            // 与「编辑」菜单。macOS 的 Cmd+C/V/X/A/Z 依赖菜单中的「编辑」项
            // （key equivalent）才能派发到 WebView 输入框；自定义菜单若缺少这些项，
            // 复制/粘贴/全选会全部失效。
            //
            // Windows/Linux 不设 app 级菜单：Tauri 的 set_menu 会把它作为原生菜单栏
            // 渲染进每个窗口（含无边框的 companion），模型顶部会多出一条菜单；
            // 而这些平台的 Ctrl+C/V 无需菜单即可生效，设置入口走托盘/右键菜单。
            #[cfg(target_os = "macos")]
            {
                let show_settings = MenuItem::with_id(
                    app,
                    "show_settings",
                    "偏好设置…",
                    true,
                    Some("CmdOrCtrl+,"),
                )?;
                let undo = PredefinedMenuItem::undo(app, None)?;
                let redo = PredefinedMenuItem::redo(app, None)?;
                let edit_sep1 = PredefinedMenuItem::separator(app)?;
                let cut = PredefinedMenuItem::cut(app, None)?;
                let copy = PredefinedMenuItem::copy(app, None)?;
                let paste = PredefinedMenuItem::paste(app, None)?;
                let select_all = PredefinedMenuItem::select_all(app, None)?;
                let edit_menu = Submenu::with_items(
                    app,
                    "编辑",
                    true,
                    &[&undo, &redo, &edit_sep1, &cut, &copy, &paste, &select_all],
                )?;
                // 退出项必须用自定义 MenuItem 而非 PredefinedMenuItem::quit：
                // 后者在 macOS 绑定原生 `terminate:`，而 terminate 会逐个询问可见窗口
                // `windowShouldClose:`——被下方 on_window_event 的 prevent_close 拦截后
                // 整个退出被取消（Cmd+Q 表现为窗口隐藏、进程残留）。自定义项直接走
                // handle_menu("quit") → app.exit(0)，绕过窗口询问，与托盘「退出」一致。
                let quit =
                    MenuItem::with_id(app, "quit", "退出 ZapMomo", true, Some("CmdOrCtrl+Q"))?;
                // 注意：muda 在 macOS 只把 Submenu 渲染为菜单栏项，顶级普通 MenuItem
                // 不显示（快捷键仍可派发）。因此偏好设置/退出须收进 app 名子菜单，
                // 保持「Apple | ZapMomo | 编辑」的 macOS 惯例结构。
                let sep = PredefinedMenuItem::separator(app)?;
                let app_submenu =
                    Submenu::with_items(app, "ZapMomo", true, &[&show_settings, &sep, &quit])?;
                let app_menu = Menu::with_items(app, &[&app_submenu, &edit_menu])?;
                app.set_menu(app_menu)?;
            }

            // 托盘菜单：显示/隐藏角色、窗口尺寸/透明度、打开设置、重启、退出。
            let tray_menu = build_tray_menu(app.handle())?;

            // 托盘图标：使用专用托盘图标（tray-icon.png）——真实应用图标的无边距版本，
            // 撑满菜单栏，避免 512px 主图标 9% 留白导致的偏小。
            let tray_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                    .expect("托盘图标加载失败");
            TrayIconBuilder::with_id(TRAY_ID)
                .icon(tray_icon)
                .menu(&tray_menu)
                .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
                .build(app)?;

            // 注册用户自定义全局快捷键（[shortcuts] 分节；单个失败仅告警）
            register_shortcuts_at_startup(app.handle());

            Ok(())
        })
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // 关闭设置/角色窗口时仅隐藏，不退出进程；退出走托盘/菜单 Cmd+Q
                //（菜单退出项须用自定义 MenuItem——原生 quit 会走 terminate: →
                //  windowShouldClose:，被本拦截器取消，见上方菜单构建处注释）。
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod companion_opacity_tests {
    use super::{clamp_opacity, opacity_from_id};

    #[test]
    fn test_opacity_from_id_mappings() {
        assert_eq!(opacity_from_id("opacity_100"), Some(1.0));
        assert_eq!(opacity_from_id("opacity_80"), Some(0.8));
        assert_eq!(opacity_from_id("opacity_60"), Some(0.6));
        assert_eq!(opacity_from_id("opacity_40"), Some(0.4));
        assert_eq!(opacity_from_id("opacity_20"), Some(0.2));
        assert_eq!(opacity_from_id("scale_100"), None);
        assert_eq!(opacity_from_id("unknown"), None);
    }

    #[test]
    fn test_clamp_opacity_bounds() {
        assert_eq!(clamp_opacity(0.05), 0.2);
        assert_eq!(clamp_opacity(-1.0), 0.2);
        assert_eq!(clamp_opacity(1.5), 1.0);
        assert_eq!(clamp_opacity(0.2), 0.2);
        assert_eq!(clamp_opacity(1.0), 1.0);
        assert_eq!(clamp_opacity(0.65), 0.65);
    }
}
