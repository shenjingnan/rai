use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::AudiocppError;
use crate::tts::config::ResolvedTtsConfig;

/// 健康检查总 deadline（含 eager 模型加载）。实测冷启动 spawn+加载 1~3s，留足余量。
const READY_TIMEOUT_SECS: u32 = 20;
/// 单次 HTTP 探测超时。
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
/// 健康轮询间隔。
const PROBE_INTERVAL: Duration = Duration::from_millis(100);
/// stderr 环形缓冲行数（错误诊断附带末尾若干行）。
const STDERR_TAIL_LINES: usize = 20;

/// sidecar 进程租约：RAII 计数，Drop 时释放；计数归零按 keepalive 策略回收。
///
/// 持有者（`AudiocppTts`）生命周期即租约生命周期：voice 会话/Announcer 常驻则
/// server 常驻；GUI 每次合成取放，配合 `set_idle_keepalive(Some(45s))` 在窗口内
/// 复用热 server（热请求 0.13s 级）。
pub struct ServerLease {
    port: u16,
    generation: u64,
}

impl ServerLease {
    /// server 基地址（`http://127.0.0.1:<port>`），HTTP 客户端直连用。
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for ServerLease {
    fn drop(&mut self) {
        release_lease(self.generation);
    }
}

struct ServerInstance {
    child: Child,
    port: u16,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    generation: u64,
}

struct ManagerState {
    instance: Option<ServerInstance>,
    lease_count: usize,
    config_hash: u64,
    orphan_reaped: bool,
}

static MANAGER: OnceLock<Mutex<ManagerState>> = OnceLock::new();
/// 空闲回收窗口（毫秒）：0 = lease 归零立即回收（CLI 语义，进程绝不残留）；
/// >0 = GUI keepalive 窗口（窗口内复用热 server）。
static IDLE_KEEPALIVE_MS: AtomicU64 = AtomicU64::new(0);
/// 实例代际：每次 spawn 递增；keepalive 线程复核时比对，避免误杀重新 spawn 的新实例。
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn manager() -> &'static Mutex<ManagerState> {
    MANAGER.get_or_init(|| {
        Mutex::new(ManagerState {
            instance: None,
            lease_count: 0,
            config_hash: 0,
            orphan_reaped: false,
        })
    })
}

/// 设置空闲回收窗口（宿主应用在启动时调用）。
///
/// GUI 传 `Some(45s)`；CLI 保持 `None`（缺省）即用即杀。
pub fn set_idle_keepalive(keepalive: Option<Duration>) {
    IDLE_KEEPALIVE_MS.store(
        keepalive.map_or(0, |d| d.as_millis().min(u64::MAX as u128) as u64),
        Ordering::SeqCst,
    );
}

/// 获取 server 租约：复用健康实例或按当前配置 spawn（模型/引擎/backend 变更自动重启）。
///
/// manager 互斥锁串行化，避免并发 lease 双 spawn；已退出实例（崩溃）懒重启。
pub fn lease(cfg: &ResolvedTtsConfig) -> Result<ServerLease, AudiocppError> {
    let engine = super::locator::locate_engine(cfg.engine_path.as_deref())?;
    let hash = config_hash(cfg, &engine);

    let mut state = manager().lock().unwrap_or_else(|e| e.into_inner());
    if !state.orphan_reaped {
        state.orphan_reaped = true;
        reap_orphan_process();
    }

    // 复用判定：实例存活（崩溃懒重启）且配置一致（模型切换自动重启）
    let exited = state.instance.as_mut().is_some_and(|inst| {
        let exited = inst.child.try_wait().map(|s| s.is_some()).unwrap_or(true);
        if exited {
            tracing::warn!(target: "audiocpp", "server 已退出（pid {}），懒重启", inst.child.id());
        }
        exited
    });
    let need_spawn = state.instance.is_none() || exited || state.config_hash != hash;
    if need_spawn {
        if let Some(mut old) = state.instance.take() {
            kill_instance(&mut old);
        }
        let inst = spawn_instance(cfg, &engine)?;
        state.config_hash = hash;
        state.instance = Some(inst);
    }

    state.lease_count += 1;
    let inst = state
        .instance
        .as_ref()
        .expect("spawn 后实例必在（上方 need_spawn 分支保证）");
    Ok(ServerLease {
        port: inst.port,
        generation: inst.generation,
    })
}

/// 显式停止 server（幂等）。GUI 退出钩子 / CLI epilogue 调用。
pub fn shutdown_blocking() {
    let mut state = manager().lock().unwrap_or_else(|e| e.into_inner());
    state.lease_count = 0;
    if let Some(mut inst) = state.instance.take() {
        kill_instance(&mut inst);
    }
}

fn release_lease(generation: u64) {
    let keepalive_ms = IDLE_KEEPALIVE_MS.load(Ordering::SeqCst);
    let mut state = manager().lock().unwrap_or_else(|e| e.into_inner());
    state.lease_count = state.lease_count.saturating_sub(1);
    if state.lease_count > 0 || state.instance.is_none() {
        return;
    }
    if keepalive_ms == 0 {
        // CLI 语义：归零立即回收，进程绝不残留
        if let Some(mut inst) = state.instance.take() {
            kill_instance(&mut inst);
        }
        return;
    }
    // GUI 语义：延迟回收。sleep 后复核「计数仍为零且实例未换代」，避免误杀
    // 窗口内新取的 lease / 崩溃重启的新实例。
    std::thread::Builder::new()
        .name("audiocpp-reaper".to_string())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(keepalive_ms));
            let mut state = manager().lock().unwrap_or_else(|e| e.into_inner());
            if state.lease_count == 0
                && state
                    .instance
                    .as_ref()
                    .is_some_and(|i| i.generation == generation)
                && let Some(mut inst) = state.instance.take()
            {
                kill_instance(&mut inst);
            }
        })
        .ok();
}

/// 决定「是否需要重启 server」的配置指纹：模型目录 / 推理后端 / 线程数 / 引擎路径。
fn config_hash(cfg: &ResolvedTtsConfig, engine: &std::path::Path) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    cfg.model_dir.hash(&mut h);
    cfg.provider.hash(&mut h);
    cfg.num_threads.hash(&mut h);
    engine.hash(&mut h);
    h.finish()
}

/// 分配空闲端口：`bind(("127.0.0.1", 0))` 取后释放（对齐 dsh 桥模式，无 rand 依赖）。
fn allocate_port() -> Result<u16, AudiocppError> {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| AudiocppError::SpawnFailed(format!("分配端口失败: {e}")))?
        .local_addr()
        .map(|a| a.port())
        .map_err(|e| AudiocppError::SpawnFailed(format!("读取端口失败: {e}")))
}

fn spawn_instance(
    cfg: &ResolvedTtsConfig,
    engine: &std::path::Path,
) -> Result<ServerInstance, AudiocppError> {
    let port = allocate_port()?;
    let config_path =
        super::server_config::write_server_config(cfg, port).map_err(AudiocppError::SpawnFailed)?;

    let mut cmd = Command::new(engine);
    cmd.arg("--config").arg(&config_path);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    // Windows：不弹控制台窗口
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AudiocppError::SpawnFailed(format!("{}: {e}", engine.display())))?;

    // stderr drain 线程：转发 tracing + 环形缓冲（错误诊断）
    let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    if let Some(stderr) = child.stderr.take() {
        let tail = stderr_tail.clone();
        std::thread::Builder::new()
            .name("audiocpp-stderr".to_string())
            .spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stderr)
                    .lines()
                    .map_while(Result::ok)
                {
                    tracing::debug!(target: "audiocpp", "{line}");
                    let mut buf = tail.lock().unwrap_or_else(|e| e.into_inner());
                    if buf.len() >= STDERR_TAIL_LINES {
                        buf.pop_front();
                    }
                    buf.push_back(line);
                }
            })
            .ok();
    }

    write_pidfile(child.id());
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let mut instance = ServerInstance {
        child,
        port,
        stderr_tail,
        generation,
    };

    // 健康检查失败 → 回收进程再返回错误（不留半启动实例）
    if let Err(e) = wait_until_ready(&mut instance) {
        kill_instance(&mut instance);
        return Err(e);
    }
    tracing::info!(target: "audiocpp", "audiocpp_server 就绪 (port {port}, generation {generation})");
    Ok(instance)
}

/// 轮询直到 `/health` 200 且 `/v1/models` 列出目标模型（eager 模式下含模型加载），
/// 或超时/进程退出。
fn wait_until_ready(inst: &mut ServerInstance) -> Result<(), AudiocppError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(PROBE_TIMEOUT)
        // 回环探测不走系统代理（代理会拦 localhost 请求返回 5xx）
        .no_proxy()
        .build()
        .map_err(|e| AudiocppError::Connection(e.to_string()))?;
    let health_url = format!("http://127.0.0.1:{}/health", inst.port);
    let models_url = format!("http://127.0.0.1:{}/v1/models", inst.port);
    let deadline = Instant::now() + Duration::from_secs(READY_TIMEOUT_SECS as u64);

    loop {
        if inst.child.try_wait().map(|s| s.is_some()).unwrap_or(true) {
            return Err(AudiocppError::SpawnFailed(format!(
                "audiocpp_server 启动后立即退出。引擎输出末尾：\n{}",
                tail_string(&inst.stderr_tail)
            )));
        }
        let healthy = client
            .get(&health_url)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if healthy {
            let listed = client
                .get(&models_url)
                .send()
                .ok()
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .and_then(|j| {
                    j["data"]
                        .as_array()
                        .map(|a| a.iter().any(|m| m["id"].as_str() == Some(super::MODEL_ID)))
                })
                .unwrap_or(false);
            if listed {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err(AudiocppError::StartupTimeout {
                timeout_secs: READY_TIMEOUT_SECS,
                stderr_tail: tail_string(&inst.stderr_tail),
            });
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}

fn kill_instance(inst: &mut ServerInstance) {
    let _ = inst.child.kill();
    let _ = inst.child.wait();
    remove_pidfile();
}

fn tail_string(tail: &Arc<Mutex<VecDeque<String>>>) -> String {
    let buf = tail.lock().unwrap_or_else(|e| e.into_inner());
    if buf.is_empty() {
        "(无输出)".to_string()
    } else {
        buf.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

/// pidfile：`<data_dir>/engines/audiocpp-server.pid`。
fn pidfile_path() -> PathBuf {
    super::locator::engines_dir().join("audiocpp-server.pid")
}

fn write_pidfile(pid: u32) {
    let _ = std::fs::create_dir_all(super::locator::engines_dir());
    let _ = std::fs::write(pidfile_path(), pid.to_string());
}

fn remove_pidfile() {
    let _ = std::fs::remove_file(pidfile_path());
}

/// 孤儿清理（宿主崩溃/强杀兜底，对齐 dsh「残留→下次启动清理」模式）：
/// 读 pidfile，pid 存活且进程名匹配 `audiocpp_server` 时 kill。不做复用
/// （manager 总是按当前配置重新生成 config 再 spawn）。
fn reap_orphan_process() {
    let Ok(content) = std::fs::read_to_string(pidfile_path()) else {
        return;
    };
    let Ok(pid) = content.trim().parse::<u32>() else {
        remove_pidfile();
        return;
    };
    let sys_pid = sysinfo::Pid::from_u32(pid);
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sys_pid]), true);
    if let Some(proc) = sys.process(sys_pid) {
        let name = proc.name().to_string_lossy().to_string();
        if name.contains("audiocpp_server") {
            tracing::warn!(target: "audiocpp", "发现残留 audiocpp_server 进程 (pid {pid})，正在清理");
            let _ = proc.kill();
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    remove_pidfile();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_port_returns_nonzero() {
        let p = allocate_port().unwrap();
        assert!(p > 0);
    }

    #[test]
    fn test_config_hash_distinguishes_model_dir() {
        let mut cfg = ResolvedTtsConfig::default();
        let engine = std::path::Path::new("/engines/audiocpp_server");
        let h1 = config_hash(&cfg, engine);
        cfg.model_dir = std::path::PathBuf::from("/models/other");
        let h2 = config_hash(&cfg, engine);
        assert_ne!(h1, h2);
        // 同配置幂等
        assert_eq!(h2, config_hash(&cfg, engine));
    }

    #[test]
    fn test_set_idle_keepalive_stores_ms() {
        set_idle_keepalive(None);
        assert_eq!(IDLE_KEEPALIVE_MS.load(Ordering::SeqCst), 0);
        set_idle_keepalive(Some(Duration::from_millis(45_000)));
        assert_eq!(IDLE_KEEPALIVE_MS.load(Ordering::SeqCst), 45_000);
        set_idle_keepalive(None);
    }

    #[test]
    fn test_shutdown_blocking_is_idempotent_when_no_instance() {
        // 无实例时调用不 panic（退出钩子可能在任何状态下触发）
        shutdown_blocking();
        shutdown_blocking();
    }
}
