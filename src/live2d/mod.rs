/// Live2D 模型配置与扫描。
///
/// 提供 Live2D 模型的配置解析与模型清单文件定位，供 CLI 与 Tauri GUI 复用。
/// 与 `kws` / `asr` 一样遵循「领域逻辑放根 crate、胶水放 src-tauri」的分层。
pub mod config;

pub use config::{Live2dFormat, ResolvedLive2dConfig};
