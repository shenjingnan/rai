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

/// 写发现文件（unix 下权限 0600；Windows 无 chmod 概念跳过）。
pub fn write_discovery(info: &DiscoveryInfo) -> Result<(), String> {
    let path = discovery_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建 runtime 目录失败: {e}"))?;
    }
    let body = serde_json::to_string(info).map_err(|e| format!("序列化发现文件失败: {e}"))?;
    std::fs::write(&path, body).map_err(|e| format!("写入发现文件失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// 删除发现文件（退出清理 / 启动清陈旧残留；不存在视为成功）。
pub fn remove_discovery() {
    let _ = std::fs::remove_file(discovery_file());
}

/// 生成一次性 token：sha256(纳秒时钟 ‖ pid ‖ 计数器) 十六进制前 32 位。
pub fn generate_token() -> String {
    use sha2::{Digest, Sha256};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(n.to_le_bytes());
    let full = hex::encode(hasher.finalize());
    full.chars().take(32).collect()
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
        last.retain(|_, t| now.duration_since(*t) < self.window);
        match last.get(&key) {
            Some(t) if now.duration_since(*t) < self.window => false,
            _ => {
                last.insert(key, now);
                true
            }
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
}
