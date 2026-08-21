//! 表演事件源抽象与驱动循环。
//!
//! [`PerformanceSource`] 是「接入任意事件源」的接口：只要产出 `(等待时长, 事件)`
//! 序列即可进入 `device-changed` 总线。当前由 [`super::TypingSimulator`] /
//! [`super::MouseSimulator`] 实现（纯模拟），未来接入 AI 代理活动翻译、LLM 编排
//! 等源时实现同一 trait 即可，消费端零改动。

use crate::performance::rng::Rng;
use crate::performance::{DeviceEvent, PerformanceScene};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// 表演事件源：产出「等待时长 + 事件」序列。`None` 表示自然结束。
pub trait PerformanceSource: Send {
    /// 本源的表演场景。
    fn scene(&self) -> PerformanceScene;

    /// 下一个事件与之前的建议等待；`None` = 表演自然结束。
    fn next_event(&mut self, rng: &mut Rng) -> Option<(Duration, DeviceEvent)>;
}

/// 可中断的停止信号：`stop()` 立即唤醒正在 `wait` 的线程，保证停止及时性。
#[derive(Clone, Default)]
pub struct StopSignal(Arc<(Mutex<bool>, Condvar)>);

impl StopSignal {
    /// 构造未触发的停止信号。
    pub fn new() -> Self {
        Self::default()
    }

    /// 发出停止信号并唤醒所有等待者；返回是否首次停止（此前未停止过）。
    pub fn stop(&self) -> bool {
        let (lock, cvar) = &*self.0;
        let mut stopped = lock.lock().unwrap();
        let first = !*stopped;
        *stopped = true;
        cvar.notify_all();
        first
    }

    /// 等待 `d`；被 `stop()` 打断返回 `false`（应停止），
    /// 等满时长返回 `true`（继续）。伪唤醒会继续等待剩余时间。
    pub fn wait(&self, d: Duration) -> bool {
        let (lock, cvar) = &*self.0;
        let mut stopped = lock.lock().unwrap();
        if *stopped {
            return false;
        }
        let deadline = Instant::now() + d;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return true;
            }
            let (guard, res) = cvar.wait_timeout(stopped, deadline - now).unwrap();
            stopped = guard;
            if *stopped {
                return false;
            }
            if res.timed_out() {
                return true;
            }
            // 伪唤醒：继续等待剩余时间
        }
    }
}

/// 通用驱动循环：逐事件先 `wait`（可被停止打断，且打断时不发该事件）再 `emit`。
///
/// 停止后绝无漏发事件：被打断的当前事件不会发出，`run_source` 立即返回。
///
/// 返回 `true` = 自然结束，`false` = 被停止。
pub fn run_source(
    src: &mut dyn PerformanceSource,
    rng: &mut Rng,
    stop: &StopSignal,
    emit: &mut dyn FnMut(&DeviceEvent),
) -> bool {
    while let Some((delay, event)) = src.next_event(rng) {
        if !stop.wait(delay) {
            return false;
        }
        emit(&event);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::{DeviceEvent, DeviceEventKind, DeviceValue};

    /// 脚本化测试源：按预定脚本产出事件后自然结束。
    struct ScriptSource {
        script: Vec<(Duration, DeviceEvent)>,
        scene: PerformanceScene,
    }

    impl ScriptSource {
        fn new(scene: PerformanceScene, script: Vec<(Duration, DeviceEvent)>) -> Self {
            Self { script, scene }
        }
    }

    impl PerformanceSource for ScriptSource {
        fn scene(&self) -> PerformanceScene {
            self.scene
        }

        fn next_event(&mut self, _rng: &mut Rng) -> Option<(Duration, DeviceEvent)> {
            if self.script.is_empty() {
                None
            } else {
                Some(self.script.remove(0))
            }
        }
    }

    fn ev(kind: DeviceEventKind, key: &str) -> DeviceEvent {
        DeviceEvent {
            kind,
            value: DeviceValue::Key(key.into()),
        }
    }

    #[test]
    fn run_source_emits_in_order_and_reports_natural_end() {
        let script = vec![
            (
                Duration::from_millis(1),
                ev(DeviceEventKind::KeyboardPress, "KeyA"),
            ),
            (
                Duration::from_millis(1),
                ev(DeviceEventKind::KeyboardRelease, "KeyA"),
            ),
        ];
        let mut src = ScriptSource::new(PerformanceScene::Typing, script);
        let mut rng = Rng::new(1);
        let stop = StopSignal::new();
        let mut seen = Vec::new();
        let natural = run_source(&mut src, &mut rng, &stop, &mut |e| {
            seen.push(e.clone());
        });
        assert!(natural, "脚本耗尽应自然结束");
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], ev(DeviceEventKind::KeyboardPress, "KeyA"));
        assert_eq!(seen[1], ev(DeviceEventKind::KeyboardRelease, "KeyA"));
    }

    #[test]
    fn run_source_reports_stopped() {
        let script = vec![
            (
                Duration::from_millis(1),
                ev(DeviceEventKind::KeyboardPress, "KeyA"),
            ),
            (
                Duration::from_millis(1),
                ev(DeviceEventKind::KeyboardPress, "KeyB"),
            ),
        ];
        let mut src = ScriptSource::new(PerformanceScene::Typing, script);
        let mut rng = Rng::new(2);
        let stop = StopSignal::new();
        let mut seen = 0;
        let result = run_source(&mut src, &mut rng, &stop, &mut |_| {
            seen += 1;
            stop.stop(); // 第一次 emit 后请求停止
        });
        assert!(!result, "被打断应返回 false");
        assert_eq!(seen, 1, "停止后的第二个事件不应发出");
    }

    #[test]
    fn stop_wakes_wait_promptly() {
        let stop = StopSignal::new();
        let started = Instant::now();
        let spawned = std::thread::spawn({
            let stop = stop.clone();
            move || {
                let ok = stop.wait(Duration::from_secs(10));
                (ok, started.elapsed())
            }
        });
        std::thread::sleep(Duration::from_millis(50));
        stop.stop();
        let (ok, elapsed) = spawned.join().unwrap();
        assert!(!ok, "被停止的 wait 应返回 false");
        assert!(
            elapsed < Duration::from_secs(1),
            "wait 应在 stop 后 ~1s 内返回，实际 {elapsed:?}"
        );
    }

    #[test]
    fn wait_returns_true_after_full_duration() {
        let stop = StopSignal::new();
        let ok = stop.wait(Duration::from_millis(10));
        assert!(ok, "未被打断且等满时长应返回 true");
    }
}
