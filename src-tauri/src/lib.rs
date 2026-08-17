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
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
use zapmomo::asr::config::AsrParamsPatch;
use zapmomo::asr::{AsrReaction, AsrResult};
use zapmomo::config::settings::{
    self, AsrSettings, CompanionWindowPosition, KwsSettings, Live2dSettings, LlmSettings,
    TtsSettings,
};
use zapmomo::kws::{KwsResult, Reaction, ReactionOutcome};
use zapmomo::llm::types::{ChatMessage, ChatRole, GenParams, InputItem, LlmParamsPatch};
use zapmomo::llm::{LlmEngine, LlmEvent};
use zapmomo::model_library;
use zapmomo::model_library::{
    InstallState as LibInstallState, LibraryModel, RuntimeAction as LibRuntimeAction,
    SetCurrentResult, SystemResources, registry::ModelType as LibModelType,
};
use zapmomo::tts::config::TtsParamsPatch;

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

/// 停止实时监听：置停止标志并等待线程退出。
#[tauri::command]
fn stop_listen(state: State<'_, ListenState>) -> Result<(), String> {
    tracing::warn!("stop_listen called (is_listening={})", state.is_listening());
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

    Ok(AsrConfigInfo {
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

/// 开始实时语音识别。
///
/// 校验模型文件后启动独立线程跑 `run_realtime_with`，识别结果经
/// `asr-result` 事件发给前端；线程结束发 `asr-stopped`。
#[tauri::command]
fn start_asr_listen(
    app: AppHandle,
    state: State<'_, AsrListenState>,
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
    Ok(())
}

/// 停止实时语音识别：置停止标志并等待线程退出。
#[tauri::command]
fn stop_asr_listen(state: State<'_, AsrListenState>) -> Result<(), String> {
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

/// 停止正在进行的合成（等待线程退出）。
#[tauri::command]
fn stop_tts(state: State<'_, TtsSynthesizeState>) -> Result<(), String> {
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
fn forward_llm_events(app: AppHandle, engine: Arc<LlmEngine>, stop_on_status: bool) {
    loop {
        match engine.try_recv() {
            Some(LlmEvent::Token(delta)) => {
                let _ = app.emit("llm-token", delta);
            }
            Some(LlmEvent::Finished(reason)) => {
                let _ = app.emit("llm-finished", reason);
                break;
            }
            Some(LlmEvent::Error(e)) => {
                let _ = app.emit("llm-error", e);
                break;
            }
            Some(LlmEvent::Status { ready }) => {
                let _ = app.emit("llm-status", LlmStatusPayload { ready });
                if stop_on_status {
                    break;
                }
            }
            None => std::thread::sleep(std::time::Duration::from_millis(10)),
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
    std::thread::spawn(move || forward_llm_events(app, engine, true));
    Ok(())
}

/// 加载 LLM 模型（异步：结果经 `llm-status`/`llm-error` 事件返回）。
#[tauri::command]
fn load_llm_model(app: AppHandle, state: State<'_, LlmState>) -> Result<(), String> {
    load_llm_impl(app, state.inner())
}

/// 卸载 LLM 模型并释放内存。
#[tauri::command]
fn unload_llm_model(state: State<'_, LlmState>) -> Result<(), String> {
    let engine = state.engine.lock().expect("llm lock poisoned").take();
    if let Some(engine) = engine {
        engine.unload().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 发起一次流式对话（token 经 `llm-token`，结束经 `llm-finished`）。
#[tauri::command]
fn chat_llm(app: AppHandle, state: State<'_, LlmState>, text: String) -> Result<(), String> {
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
    std::thread::spawn(move || forward_llm_events(app, engine, false));
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

/// 持久化「启用 KWS」开关，写入 `[kws].enabled`（缺省 false）。
/// 开关只持久化偏好；立即开始/停止监听由前端调用 `start_listen` / `stop_listen`，
/// 下次启动自动监听由 `.setup()` 判断 `[kws].enabled` 触发。
#[tauri::command]
fn set_kws_enabled(enabled: bool) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let kws = settings.kws.get_or_insert_with(KwsSettings::default);
    kws.enabled = Some(enabled);
    settings::save_settings(&settings)?;
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
#[tauri::command]
fn set_microphone(mic: String) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    settings.microphone = if mic.trim().is_empty() {
        None
    } else {
        Some(mic.trim().to_string())
    };
    settings::save_settings(&settings)
}

/// GUI 展示用的 Live2D 配置信息。
#[derive(Serialize)]
struct Live2dConfigInfo {
    model_dir: Option<String>,
    model_file: Option<String>,
    format: Option<String>,
    models_present: bool,
    window_scale: Option<f64>,
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

    Ok(Live2dConfigInfo {
        model_dir: Some(cfg.model_dir.display().to_string()),
        model_file: cfg.model_file.map(|p| p.display().to_string()),
        format: cfg.format.map(|f| f.to_str().to_string()),
        models_present,
        window_scale: live2d_settings.and_then(|l| l.window_scale),
        settings_path: settings::get_settings_path().display().to_string(),
    })
}

/// 选择模型目录后返回的模型信息。
#[derive(Clone, Serialize)]
struct Live2dModelInfo {
    model_dir: String,
    model_file: String,
    format: String,
}

/// 校验并持久化用户选择的 Live2D 模型目录，放行 asset 协议 scope。
#[tauri::command]
fn set_live2d_model(app: AppHandle, dir: String) -> Result<Live2dModelInfo, String> {
    let dir_path = std::path::PathBuf::from(&dir);
    let (model_file, format) = zapmomo::live2d::config::find_model_file(&dir_path)
        .ok_or_else(|| "目录中未找到 Live2D 模型清单（*.model3.json 或 model.json）".to_string())?;

    // 前端渲染使用 pixi-live2d-display/cubism4，仅支持 Cubism 3/4/5（.moc3），
    // 不支持 Cubism 2（.moc），这里提前拒绝避免前端静默失败。
    if format == zapmomo::live2d::config::Live2dFormat::Cubism2 {
        return Err(
            "暂不支持 Cubism 2 模型（.moc + model.json），请使用 Cubism 3/4/5 模型（.moc3 + .model3.json）"
                .to_string(),
        );
    }

    app.asset_protocol_scope()
        .allow_directory(&dir_path, true)
        .map_err(|e| format!("无法放行模型目录: {e}"))?;

    let mut settings = settings::load_settings()?.unwrap_or_default();
    // 仅更新 model_dir，保留已有的 window_position 等字段。
    let live2d = settings.live2d.get_or_insert_with(Live2dSettings::default);
    live2d.model_dir = Some(dir.clone());
    settings::save_settings(&settings)?;

    let info = Live2dModelInfo {
        model_dir: dir,
        model_file: model_file.display().to_string(),
        format: format.to_str().to_string(),
    };
    // 通知常驻角色窗口即时重载新模型（同进程事件，跨窗口同步）。
    let _ = app.emit("live2d-model-changed", &info);
    Ok(info)
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

/// 设置并持久化角色窗口缩放比例（1.0 = 100%）。
///
/// 由设置面板（或角色窗口自身）调用：写入 `~/.zapmomo/settings.toml` 的
/// `[live2d.window_scale]` 段，并通过 `companion-scale-changed` 事件通知角色窗口
/// （角色窗口持有真实模型宽高比，负责把比例换算成绝对尺寸并 `setSize`）。
#[tauri::command]
fn set_companion_scale(app: AppHandle, scale: f64) -> Result<(), String> {
    apply_companion_scale(&app, scale)
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
        "quit" => app.exit(0),
        _ => {
            if let Some(scale) = scale_from_id(id) {
                let _ = apply_companion_scale(app, scale);
            }
        }
    }
}

/// 构建角色窗口的右键菜单（窗口尺寸子菜单 + 打开设置 / 隐藏角色 / 退出）。
fn build_companion_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let s25 = MenuItem::with_id(app, "scale_25", "25%", true, None::<&str>)?;
    let s50 = MenuItem::with_id(app, "scale_50", "50%", true, None::<&str>)?;
    let s70 = MenuItem::with_id(app, "scale_70", "70%", true, None::<&str>)?;
    let s100 = MenuItem::with_id(app, "scale_100", "100%", true, None::<&str>)?;
    let s150 = MenuItem::with_id(app, "scale_150", "150%", true, None::<&str>)?;
    let s200 = MenuItem::with_id(app, "scale_200", "200%", true, None::<&str>)?;
    let scale_submenu = Submenu::with_items(
        app,
        "窗口尺寸",
        true,
        &[&s25, &s50, &s70, &s100, &s150, &s200],
    )?;
    let open_settings = MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide_companion", "隐藏角色", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    Menu::with_items(app, &[&scale_submenu, &open_settings, &hide, &quit])
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

/// 从模型库列表解析模型（registry 或 standalone external）。
fn resolve_library_model(id: &str) -> Result<LibraryModel, String> {
    model_library::list_models()
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("未知的模型：{id}"))
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
            std::thread::spawn(move || forward_llm_events(app, new_engine, true));
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
                    std::thread::spawn(move || forward_llm_events(app, old_engine, true));
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
    let reg = model_library::registry::model_by_id(&id)
        .ok_or_else(|| format!("未知的 Registry 模型：{id}"))?;
    let dir = model_library::managed_install_dir(reg);
    if dir.exists() {
        model_library::delete_managed_dir(&dir)?;
    }
    Ok(())
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
        .manage(ListenState::new())
        .manage(DownloadState::default())
        .manage(AsrListenState::new())
        .manage(AsrDownloadState::default())
        .manage(TtsSynthesizeState::new())
        .manage(TtsDownloadState::default())
        .manage(LlmState::new())
        .manage(ModelLibraryState::default())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            list_devices,
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
            list_model_library,
            get_system_resources,
            download_library_model,
            cancel_model_download,
            set_current_model,
            delete_model,
            remove_local_model,
            add_local_model,
            open_model_directory,
            get_live2d_config,
            set_live2d_model,
            save_companion_position,
            set_companion_scale,
            show_companion_menu,
            get_hide_dock_icon,
            set_hide_dock_icon,
            open_settings,
            hide_companion,
            quit_app
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

            // 启动自动加载 LLM 模型（若用户开启 auto_load）：后台异步加载，失败静默降级为手动加载。
            if llm_resolved_config().map(|c| c.auto_load).unwrap_or(false) {
                let handle = app.handle().clone();
                let state = app.state::<LlmState>();
                if let Err(e) = load_llm_impl(handle, state.inner()) {
                    tracing::warn!("自动加载 LLM 失败: {e}");
                }
            }

            // 启动自动监听 KWS（若用户启用 KWS）：后台线程监听，失败静默降级为手动启动。
            // 使用持久化的麦克风（顶层 microphone）与自定义唤醒词（[kws].custom_keywords，空则模型内置）。
            if zapmomo::kws::config::resolve(loaded.as_ref().and_then(|s| s.kws.as_ref()), None)
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

            let mut companion = WebviewWindowBuilder::new(
                app,
                "companion",
                WebviewUrl::App("companion.html".into()),
            )
            .title("Zap Momo")
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

            // 设置窗口：默认隐藏，由 cmd+, 或托盘菜单打开；关闭时隐藏而非退出。
            let mut settings =
                WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
                    .title("Zap Momo 设置")
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
            #[cfg(not(target_os = "macos"))]
            {
                settings = settings.decorations(false).transparent(true);
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

            // 应用菜单：偏好设置…（cmd+,）、编辑菜单与退出（Cmd+Q）。
            // macOS 的 Cmd+C/V/X/A/Z 依赖菜单中的「编辑」项（key equivalent）才能派发到
            // WebView 输入框；自定义菜单若缺少这些项，复制/粘贴/全选会全部失效。
            let show_settings =
                MenuItem::with_id(app, "show_settings", "偏好设置…", true, Some("CmdOrCtrl+,"))?;
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
            let app_menu = Menu::with_items(
                app,
                &[
                    &show_settings,
                    &edit_menu,
                    &PredefinedMenuItem::quit(app, None)?,
                ],
            )?;
            app.set_menu(app_menu)?;

            // 托盘菜单：显示/隐藏角色、打开设置、退出。
            let toggle_companion =
                MenuItem::with_id(app, "toggle_companion", "显示/隐藏角色", true, None::<&str>)?;
            let open_settings =
                MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&toggle_companion, &open_settings, &quit])?;

            // 托盘图标：使用专用托盘图标（tray-icon.png）——真实应用图标的无边距版本，
            // 撑满菜单栏，避免 512px 主图标 9% 留白导致的偏小。
            let tray_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                    .expect("托盘图标加载失败");
            TrayIconBuilder::new()
                .icon(tray_icon)
                .menu(&tray_menu)
                .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
                .build(app)?;

            Ok(())
        })
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // 关闭设置/角色窗口时仅隐藏，不退出进程；退出走托盘/菜单 Cmd+Q。
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
