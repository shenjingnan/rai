//! ai-rust-starter 桌面应用（Tauri 2）。
//!
//! 复用根 crate `ai-rust-starter` 的 KWS / 音频 / 配置逻辑：
//! - 通过 Tauri command 暴露设备列表、KWS 配置、开始/停止监听；
//! - 监听循环跑在独立 `std::thread`，检测到唤醒词经 `TauriReaction`
//!   以 `kws-detected` 事件推给前端；结束（正常/出错/手动停止）发 `kws-stopped`。
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ai_rust_starter::kws::{KwsResult, Reaction, ReactionOutcome};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

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
    ai_rust_starter::audio::list_input_devices()
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
    settings_path: String,
}

/// 读取合并后的 KWS 配置（settings.toml + 默认值），并给出模型是否就绪。
#[tauri::command]
fn get_kws_config() -> Result<KwsConfigInfo, String> {
    let settings = ai_rust_starter::config::settings::load_settings()?;
    let kws_settings = settings.as_ref().and_then(|s| s.kws.clone());
    let cfg = ai_rust_starter::kws::config::resolve(kws_settings.as_ref(), None)?;

    let files = [
        &cfg.encoder,
        &cfg.decoder,
        &cfg.joiner,
        &cfg.tokens,
        &cfg.keywords_file,
    ];
    let models_present = files.iter().all(|p| p.is_file());
    let keywords =
        ai_rust_starter::kws::config::parse_keywords_file(&cfg.keywords_file).unwrap_or_default();

    Ok(KwsConfigInfo {
        model_dir: cfg.model_dir.display().to_string(),
        provider: cfg.provider.clone(),
        num_threads: cfg.num_threads,
        sample_rate: cfg.sample_rate,
        keywords,
        models_present,
        settings_path: ai_rust_starter::config::settings::get_settings_path()
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

    let settings = ai_rust_starter::config::settings::load_settings()?;
    let kws_settings = settings.as_ref().and_then(|s| s.kws.clone());
    let cfg = ai_rust_starter::kws::config::resolve(kws_settings.as_ref(), None)?;

    // 同步校验/编码附加关键词（原始中文自动转 ppinyin），避免编码失败时空指针崩溃
    if let Some(k) = keywords.as_deref() {
        ai_rust_starter::kws::token::encode_custom_keywords(k, &cfg.tokens)?;
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
            "缺少模型文件: {}\n\n请在 {} 中配置 [kws] model_dir，\n或先在项目目录运行 scripts/download-kws-model.sh 下载模型。",
            missing.display(),
            ai_rust_starter::config::settings::get_settings_path().display()
        ));
    }

    let running = state.running.clone();
    running.store(true, Ordering::Relaxed);
    let thread_app = app.clone();
    let handle = std::thread::spawn(move || {
        tracing::info!("KWS listen thread started");
        let mut reaction = TauriReaction { app: thread_app };
        let result = ai_rust_starter::kws::run_realtime_with(
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

/// Tauri 应用入口。
pub fn run() {
    ai_rust_starter::logging::init_logging();
    tauri::Builder::default()
        .manage(ListenState::new())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            list_devices,
            get_kws_config,
            start_listen,
            stop_listen,
            is_listening
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
