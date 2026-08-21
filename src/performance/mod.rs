//! BongoCat 兼容表演系统（仅模拟，绝不监听真实键鼠）。
//!
//! 让桌宠 Live2D 模型「自己动起来」，复刻 [BongoCat](https://github.com/ayangweb/BongoCat)
//! 的打字与鼠标效果。核心思路：保留 BongoCat 的 `device-changed` 事件总线与消费端
//! 全部约定，仅替换事件生产端——由 `PerformanceSource` 产出虚拟键鼠事件，而非 rdev
//! 监听用户真实输入。
//!
//! **绝不监听真实键鼠**：本模块不依赖任何全局输入钩子，所有事件仅由
//! [`TypingSimulator`] / [`MouseSimulator`]（或未来接入的 AI 代理源）模拟产生。
//!
//! # 事件格式
//!
//! [`DeviceEvent`] 与 BongoCat `src-tauri/src/core/device.rs` 逐字节同构：
//!
//! ```json
//! { "kind": "KeyboardPress", "value": "KeyA" }
//! { "kind": "MouseMove", "value": { "x": 960.4, "y": 540.2 } }
//! ```

pub mod rng;
pub mod simulator;
pub mod source;

pub use rng::Rng;
pub use simulator::{KeyPool, MouseSimulator, TypingSimulator};
pub use source::{PerformanceSource, StopSignal, run_source};

use serde::{Deserialize, Serialize};

/// 表演场景。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceScene {
    Typing,
    Mouse,
    /// 键鼠同动（BongoCat 标准模式完整观感：左手敲键盘 + 右手鼠标 + 头部跟随）。
    Both,
}

impl PerformanceScene {
    /// 稳定字符串标识（与 serde 序列化一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            PerformanceScene::Typing => "typing",
            PerformanceScene::Mouse => "mouse",
            PerformanceScene::Both => "both",
        }
    }

    /// 是否包含键盘通道。
    pub fn has_typing(self) -> bool {
        matches!(self, PerformanceScene::Typing | PerformanceScene::Both)
    }

    /// 是否包含鼠标通道。
    pub fn has_mouse(self) -> bool {
        matches!(self, PerformanceScene::Mouse | PerformanceScene::Both)
    }
}

/// 键鼠事件种类，序列化名称与 BongoCat 完全一致（PascalCase）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceEventKind {
    MousePress,
    MouseRelease,
    MouseMove,
    KeyboardPress,
    KeyboardRelease,
}

/// 事件值：键名（rdev Debug 串，如 `"KeyA"`）或鼠标坐标对象。
///
/// `#[serde(untagged)]` 精确复刻 BongoCat `value: serde_json::Value` 的双形状：
/// 字符串与 `{x, y}` 对象。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeviceValue {
    Key(String),
    Point { x: f64, y: f64 },
}

/// 单个键鼠事件（与 BongoCat `DeviceEvent` 同构）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceEvent {
    pub kind: DeviceEventKind,
    pub value: DeviceValue,
}

impl DeviceEvent {
    pub fn key(kind: DeviceEventKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            value: DeviceValue::Key(key.into()),
        }
    }

    pub fn point(x: f64, y: f64) -> Self {
        Self {
            kind: DeviceEventKind::MouseMove,
            value: DeviceValue::Point { x, y },
        }
    }
}

/// 键鼠活动区域（物理像素）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// 坐标是否落在区域内（含边界）。
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_event_serializes_like_bongocat() {
        let ev = DeviceEvent::key(DeviceEventKind::KeyboardPress, "KeyA");
        let s = serde_json::to_string(&ev).unwrap();
        assert_eq!(s, r#"{"kind":"KeyboardPress","value":"KeyA"}"#);
    }

    #[test]
    fn mouse_move_event_serializes_like_bongocat() {
        let ev = DeviceEvent::point(960.5, 540.25);
        let s = serde_json::to_string(&ev).unwrap();
        assert_eq!(s, r#"{"kind":"MouseMove","value":{"x":960.5,"y":540.25}}"#);
    }

    #[test]
    fn value_round_trips_untagged_shapes() {
        let key: DeviceValue = serde_json::from_str(r#""KeyA""#).unwrap();
        assert_eq!(key, DeviceValue::Key("KeyA".into()));

        let pt: DeviceValue = serde_json::from_str(r#"{"x":1.0,"y":2.0}"#).unwrap();
        assert_eq!(pt, DeviceValue::Point { x: 1.0, y: 2.0 });
    }

    #[test]
    fn scene_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&PerformanceScene::Typing).unwrap(),
            r#""typing""#
        );
        assert_eq!(
            serde_json::to_string(&PerformanceScene::Mouse).unwrap(),
            r#""mouse""#
        );
        assert_eq!(PerformanceScene::Mouse.as_str(), "mouse");
    }

    #[test]
    fn both_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&PerformanceScene::Both).unwrap(),
            r#""both""#
        );
        assert_eq!(PerformanceScene::Both.as_str(), "both");
    }

    #[test]
    fn scene_has_channel_flags() {
        assert!(PerformanceScene::Typing.has_typing());
        assert!(!PerformanceScene::Typing.has_mouse());
        assert!(!PerformanceScene::Mouse.has_typing());
        assert!(PerformanceScene::Mouse.has_mouse());
        assert!(PerformanceScene::Both.has_typing());
        assert!(PerformanceScene::Both.has_mouse());
    }

    #[test]
    fn rect_contains_boundary() {
        let r = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        };
        assert!(r.contains(0.0, 0.0));
        assert!(r.contains(100.0, 50.0));
        assert!(!r.contains(100.1, 25.0));
        assert!(!r.contains(-0.1, 25.0));
    }
}
