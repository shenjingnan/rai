/// dsh 桥：接收 deepseek-harness 插件推送的任务事件（loopback HTTP 直推）。
///
/// ZapMomo 在 app 进程内起一个仅绑 127.0.0.1 的极小 HTTP 服务；dsh 侧 Cordis 插件
/// 在任务状态翻转瞬间 POST 语义化事件（`POST /dsh/events` + Bearer token），毫秒级
/// 到达、无轮询。端口与 token 写入发现文件 `~/.zapmomo/runtime/dsh-bridge.json`
/// （权限 0600），插件每次发送前现读；ZapMomo 未运行时插件静默跳过。
pub mod config;
pub mod event;
pub mod lines;

use crate::config::settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 事件载荷大小上限（超出回 413；Task 5 的 serve 使用）。
pub const MAX_BODY_BYTES: u64 = 64 * 1024;
/// HTTP recv 超时 = 停止标志检查周期（Task 5 的 serve 使用）。
pub const RECV_TIMEOUT: Duration = Duration::from_millis(200);

/// 发现文件路径：`~/.zapmomo/runtime/dsh-bridge.json`。
pub fn discovery_file() -> std::path::PathBuf {
    settings::get_settings_dir()
        .join("runtime")
        .join("dsh-bridge.json")
}

/// 发现文件内容（dsh 插件读取以定位桥端口与鉴权 token）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryInfo {
    pub port: u16,
    pub token: String,
}

/// 写发现文件（tmp + rename 原子写，unix 下权限 0600；Windows 无 chmod 概念跳过）。
pub fn write_discovery(info: &DiscoveryInfo) -> Result<(), String> {
    let path = discovery_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建 runtime 目录失败: {e}"))?;
    }
    let body = serde_json::to_string(info).map_err(|e| format!("序列化发现文件失败: {e}"))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &body).map_err(|e| format!("写入发现文件失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("替换发现文件失败: {e}"))?;
    Ok(())
}

/// 删除发现文件（退出清理 / 启动清陈旧残留；不存在视为成功）。
pub fn remove_discovery() {
    if let Err(e) = std::fs::remove_file(discovery_file()) {
        tracing::debug!("删除发现文件失败: {e}");
    }
}

/// 生成一次性 token：sha256(纳秒时钟 ‖ pid ‖ 计数器) 十六进制前 32 位。
pub fn generate_token() -> String {
    use sha2::{Digest, Sha256};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0); // 时钟异常（1970 年前）退化为 0；pid + 计数器仍保证唯一性
    let mut hasher = Sha256::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(n.to_le_bytes());
    let full = hex::encode(hasher.finalize());
    full[..32].to_string()
}

/// (session_id, 事件类型) 级别节流：窗口内重复事件直接丢弃。
///
/// 事件风暴 / dsh 重启重放的护栏；顺带清理过期项防 map 无界增长。
pub struct EventThrottle {
    window: Duration,
    last: Mutex<HashMap<(String, &'static str), Instant>>,
}

impl EventThrottle {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            last: Mutex::new(HashMap::new()),
        }
    }

    /// 该事件是否放行（窗口内同 (session, kind) 重复 -> false）。
    pub fn allow(&self, event: &event::DshEvent) -> bool {
        let key = (event.session_id().to_string(), event.kind());
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        // 清理过期条目，防 map 无界增长
        last.retain(|_, t| now.duration_since(*t) < self.window);
        // 判断当前 key 是否在窗口内重复
        match last.get(&key) {
            Some(t) if now.duration_since(*t) < self.window => false,
            _ => {
                last.insert(key, now);
                true
            }
        }
    }
}

/// 桥服务主循环：绑定 loopback -> `on_ready(实际端口)` -> 循环收事件交给 sink。
///
/// - `port == 0` 绑随机端口（推荐，避免冲突）
/// - `running == false` 后最多一个 [`RECV_TIMEOUT`] 周期内退出
/// - 事件处理（节流/台词/分发）在 sink 闭包内，本层只管 HTTP 语义
pub fn serve(
    port: u16,
    token: &str,
    sink: &mut dyn FnMut(event::DshEvent),
    running: &std::sync::atomic::AtomicBool,
    on_ready: &mut dyn FnMut(u16),
) -> Result<(), String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("绑定 127.0.0.1:{port} 失败: {e}"))?;
    let actual = listener
        .local_addr()
        .map_err(|e| format!("获取监听端口失败: {e}"))?
        .port();
    let server = tiny_http::Server::from_listener(listener, None)
        .map_err(|e| format!("启动 HTTP 服务失败: {e}"))?;
    on_ready(actual);
    tracing::info!("dsh 桥监听 127.0.0.1:{actual}");

    loop {
        if !running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
        match server.recv_timeout(RECV_TIMEOUT) {
            Ok(Some(mut request)) => {
                let status = handle_request(&mut request, token, sink);
                let _ = request.respond(tiny_http::Response::empty(status));
            }
            Ok(None) => {} // 超时：回循环头检查 running
            Err(e) => tracing::warn!("dsh 桥接收请求异常: {e}"),
        }
    }
}

/// 处理单条请求，返回响应状态码。
fn handle_request(
    request: &mut tiny_http::Request,
    token: &str,
    sink: &mut dyn FnMut(event::DshEvent),
) -> u16 {
    // 仅接受 POST 方法
    if request.method() != &tiny_http::Method::Post {
        return 405;
    }
    // 仅接受 /dsh/events 路径
    if request.url() != "/dsh/events" {
        return 404;
    }
    // 验证 Bearer token
    let expected = format!("Bearer {token}");
    let authorized = request
        .headers()
        .iter()
        .any(|h| h.field.equiv("Authorization") && h.value.as_str() == expected);
    if !authorized {
        return 401;
    }
    // 拒绝超大载荷
    if request
        .body_length()
        .is_some_and(|len| len as u64 > MAX_BODY_BYTES)
    {
        return 413;
    }
    // 读取请求体（上限 MAX_BODY_BYTES，手动限制因 take 不可用于 trait object）
    let mut body = String::new();
    {
        let mut buf = [0u8; 4096];
        let mut total: u64 = 0;
        let reader = request.as_reader();
        loop {
            match std::io::Read::read(reader, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    total += n as u64;
                    if total > MAX_BODY_BYTES {
                        return 413;
                    }
                    body.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
                Err(_) => return 400,
            }
        }
    }
    // 解析事件并分发到 sink
    match event::parse_event(&body) {
        Ok(Some(ev)) => {
            sink(ev);
            204
        }
        Ok(None) => {
            tracing::debug!(
                "dsh 桥忽略未知类型事件: {}",
                body.chars().take(200).collect::<String>()
            );
            204
        }
        Err(e) => {
            tracing::warn!("dsh 桥事件解析失败: {e}");
            400
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsh::event::DshEvent;
    use crate::test_util::run_with_temp_home;

    #[test]
    fn test_discovery_roundtrip_and_permissions() {
        run_with_temp_home(|_| {
            let info = DiscoveryInfo {
                port: 47800,
                token: "abc".to_string(),
            };
            write_discovery(&info).unwrap();
            let read: DiscoveryInfo =
                serde_json::from_str(&std::fs::read_to_string(discovery_file()).unwrap()).unwrap();
            assert_eq!(read.port, 47800);
            assert_eq!(read.token, "abc");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(discovery_file())
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o600, "发现文件权限应为 0600");
            }
            remove_discovery();
            assert!(!discovery_file().exists());
        });
    }

    #[test]
    fn test_generate_token_shape() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 32, "token 应为 32 位 hex");
        assert_ne!(a, b, "连续生成的 token 应不同");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_throttle_blocks_same_key_within_window() {
        let t = EventThrottle::new(std::time::Duration::from_secs(3));
        let ev = DshEvent::TaskStarted {
            session_id: "s1".to_string(),
            title: None,
        };
        assert!(t.allow(&ev), "首次应放行");
        assert!(!t.allow(&ev), "窗口内同 (session, kind) 应拦截");
    }

    #[test]
    fn test_throttle_different_keys_pass() {
        let t = EventThrottle::new(std::time::Duration::from_secs(3));
        let a = DshEvent::TaskStarted {
            session_id: "s1".to_string(),
            title: None,
        };
        let b = DshEvent::TaskStarted {
            session_id: "s2".to_string(),
            title: None,
        };
        let c = DshEvent::TaskFinished {
            session_id: "s1".to_string(),
            title: None,
            reason: None,
        };
        assert!(t.allow(&a));
        assert!(t.allow(&b), "不同 session 不拦截");
        assert!(t.allow(&c), "同 session 不同类型不拦截");
    }

    #[test]
    fn test_throttle_allows_after_window() {
        let t = EventThrottle::new(std::time::Duration::from_millis(20));
        let ev = DshEvent::TaskStarted {
            session_id: "s1".to_string(),
            title: None,
        };
        assert!(t.allow(&ev));
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(t.allow(&ev), "窗口过后应放行");
    }

    #[test]
    fn test_throttle_empty_session_id_shared_key() {
        let t = EventThrottle::new(std::time::Duration::from_secs(3));
        let a = DshEvent::TaskStarted {
            session_id: String::new(),
            title: None,
        };
        assert!(t.allow(&a), "空 session 首次应放行");
        assert!(!t.allow(&a), "空 session 同类型窗口内应拦截");
    }

    /// 集成测试：serve 桥服务主循环的 HTTP 语义（401/400/404/405/204）与停止行为。
    #[test]
    fn test_serve_roundtrip() {
        run_with_temp_home(|_| {
            let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let (event_tx, event_rx) = std::sync::mpsc::channel::<DshEvent>();
            let (ready_tx, ready_rx) = std::sync::mpsc::channel::<u16>();
            let r = running.clone();
            let handle = std::thread::spawn(move || {
                let mut sink = move |ev: DshEvent| {
                    let _ = event_tx.send(ev);
                };
                let mut on_ready = |port: u16| {
                    let _ = ready_tx.send(port);
                };
                serve(0, "test-token", &mut sink, &r, &mut on_ready).unwrap();
            });

            let port = ready_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("on_ready 应回报端口");

            // 创建不把非 2xx 当错误的 agent，方便断言所有状态码
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .build()
                .into();

            // 有效事件 -> 204 且 sink 收到
            let resp = agent
                .post(&format!("http://127.0.0.1:{port}/dsh/events"))
                .header("Authorization", "Bearer test-token")
                .send(r#"{"type":"task-started","session_id":"s1","title":"修 bug"}"#)
                .unwrap();
            assert_eq!(resp.status(), 204);
            let ev = event_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(
                ev,
                DshEvent::TaskStarted {
                    session_id: "s1".to_string(),
                    title: Some("修 bug".to_string()),
                }
            );

            // 错 token -> 401
            let resp = agent
                .post(&format!("http://127.0.0.1:{port}/dsh/events"))
                .header("Authorization", "Bearer wrong")
                .send(r#"{"type":"task-started","session_id":"s1"}"#)
                .unwrap();
            assert_eq!(resp.status(), 401);

            // 坏 JSON -> 400
            let resp = agent
                .post(&format!("http://127.0.0.1:{port}/dsh/events"))
                .header("Authorization", "Bearer test-token")
                .send("not-json")
                .unwrap();
            assert_eq!(resp.status(), 400);

            // 未知 type -> 204 但不产生事件
            let resp = agent
                .post(&format!("http://127.0.0.1:{port}/dsh/events"))
                .header("Authorization", "Bearer test-token")
                .send(r#"{"type":"future-event","session_id":"s1"}"#)
                .unwrap();
            assert_eq!(resp.status(), 204);
            // 使用 recv_timeout 断言超时，避免前序 POST 排队事件干扰
            assert!(
                event_rx.recv_timeout(Duration::from_millis(300)).is_err(),
                "未知类型不应产生事件"
            );

            // 未知路径 -> 404
            let resp = agent
                .post(&format!("http://127.0.0.1:{port}/other"))
                .header("Authorization", "Bearer test-token")
                .send("")
                .unwrap();
            assert_eq!(resp.status(), 404);

            // 非 POST -> 405
            let resp = agent
                .get(&format!("http://127.0.0.1:{port}/dsh/events"))
                .call()
                .unwrap();
            assert_eq!(resp.status(), 405);

            // 停止：running=false 后线程应在 ~1 个 RECV_TIMEOUT 内退出
            running.store(false, std::sync::atomic::Ordering::Relaxed);
            handle.join().unwrap();
        });
    }
}
