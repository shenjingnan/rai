//! ZapMomo 桌面应用（Tauri 2）。
//!
//! 复用根 crate `zapmomo` 的 KWS / 音频 / 配置逻辑：
//! - 通过 Tauri command 暴露设备列表、KWS 配置、开始/停止监听；
//! - 监听循环跑在独立 `std::thread`，检测到唤醒词经 `TauriReaction`
//!   以 `kws-detected` 事件推给前端；结束（正常/出错/手动停止）发 `kws-stopped`。
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
use zapmomo::asr::{AsrReaction, AsrResult};
use zapmomo::config::settings::{self, CompanionWindowPosition, Live2dSettings, LlmSettings};
use zapmomo::kws::{KwsResult, Reaction, ReactionOutcome};
use zapmomo::llm::types::{ChatMessage, ChatRole};
use zapmomo::llm::{LlmEngine, LlmEvent};

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

/// 监听线程状态：共享停止标志 + 线程句柄。
struct ListenState {
    running: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl ListenState {
    fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        }
    }

    fn is_listening(&self) -> bool {
        self.running.load(Ordering::Relaxed)
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
    model_dir: String,
    provider: String,
    num_threads: i32,
    sample_rate: i32,
    keywords: Vec<String>,
    models_present: bool,
    model_downloading: bool,
    settings_path: String,
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
        model_dir: cfg.model_dir.display().to_string(),
        provider: cfg.provider.clone(),
        num_threads: cfg.num_threads,
        sample_rate: cfg.sample_rate,
        keywords,
        models_present,
        model_downloading: state.in_progress.load(Ordering::Relaxed),
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
    })
}

/// 开始实时监听唤醒词。
///
/// 校验模型文件后启动独立线程跑 `run_realtime_with`，检测结果经
/// `kws-detected` 事件发给前端；线程结束发 `kws-stopped`。
#[tauri::command]
fn start_listen(
    app: AppHandle,
    state: State<'_, ListenState>,
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
    let thread_app = app.clone();
    let handle = std::thread::spawn(move || {
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
}

impl AsrListenState {
    fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        }
    }

    fn is_listening(&self) -> bool {
        self.running.load(Ordering::Relaxed)
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

/// GUI 展示用的 ASR 配置信息。
#[derive(Serialize)]
struct AsrConfigInfo {
    model_dir: String,
    provider: String,
    num_threads: i32,
    sample_rate: i32,
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
    let thread_app = app.clone();
    let handle = std::thread::spawn(move || {
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
    models_present: bool,
    model_downloading: bool,
    settings_path: String,
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
        models_present,
        model_downloading: state.in_progress.load(Ordering::Relaxed),
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
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

/// 列出模型包内置的参考音色。
#[tauri::command]
fn list_tts_voices() -> Result<Vec<zapmomo::tts::TtsVoice>, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    let cfg = zapmomo::tts::config::resolve(tts_settings.as_ref(), None)?;
    Ok(zapmomo::tts::voice::list_builtin_voices(&cfg.model_dir))
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
}

impl LlmState {
    fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(None)),
        }
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
    enable_thinking: bool,
    auto_load: bool,
    settings_path: String,
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
    Ok(LlmConfigInfo {
        enabled: cfg.enabled,
        provider: cfg.provider,
        model_path: cfg.model_path.display().to_string(),
        models_present: cfg.model_path.is_file(),
        ready,
        enable_thinking: cfg.params.enable_thinking,
        auto_load: cfg.auto_load,
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
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
    let messages = vec![ChatMessage::new(ChatRole::User, text)];
    engine
        .generate(messages, cfg.params)
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
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            list_devices,
            get_kws_config,
            start_listen,
            stop_listen,
            is_listening,
            download_kws_model,
            get_asr_config,
            start_asr_listen,
            stop_asr_listen,
            is_asr_listening,
            download_asr_model,
            get_tts_config,
            list_tts_voices,
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
            get_live2d_config,
            set_live2d_model,
            save_companion_position,
            set_companion_scale,
            show_companion_menu,
            open_settings,
            hide_companion,
            quit_app
        ])
        .setup(|app| {
            // macOS：转成纯桌面摆件（accessory），从 Dock 与 Cmd+Tab 中消失，仅保留托盘入口。
            // 设置窗口仍通过 set_focus 正常激活（tao 内部会 activateIgnoringOtherApps）。
            #[cfg(target_os = "macos")]
            {
                app.handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory)?;
            }

            // 启动自动加载 LLM 模型（若用户开启 auto_load）：后台异步加载，失败静默降级为手动加载。
            if llm_resolved_config().map(|c| c.auto_load).unwrap_or(false) {
                let handle = app.handle().clone();
                let state = app.state::<LlmState>();
                if let Err(e) = load_llm_impl(handle, state.inner()) {
                    tracing::warn!("自动加载 LLM 失败: {e}");
                }
            }

            // 常驻角色窗口：透明、无边框、永远置顶、不入任务栏，静态展示 Live2D。
            // 读一次 settings：同时恢复记忆的尺寸与位置。
            let loaded = settings::load_settings().ok().flatten();
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
                    .inner_size(760.0, 600.0)
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

            // 应用菜单：偏好设置…（cmd+,）与退出（Cmd+Q）。
            let show_settings =
                MenuItem::with_id(app, "show_settings", "偏好设置…", true, Some("CmdOrCtrl+,"))?;
            let app_menu = Menu::with_items(
                app,
                &[&show_settings, &PredefinedMenuItem::quit(app, None)?],
            )?;
            app.set_menu(app_menu)?;

            // 托盘菜单：显示/隐藏角色、打开设置、退出。
            let toggle_companion =
                MenuItem::with_id(app, "toggle_companion", "显示/隐藏角色", true, None::<&str>)?;
            let open_settings =
                MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&toggle_companion, &open_settings, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("缺少默认窗口图标").clone())
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
