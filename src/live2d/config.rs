/// Live2D 模型配置解析与扫描。
///
/// 负责把 `settings.toml` 的 `[live2d]` 表解析成 `ResolvedLive2dConfig`，
/// 以及在用户选择的目录里定位模型清单文件（`.model3.json` / `model.json`）。
use crate::config::settings::{Live2dSettings, resolve_env_ref};
use std::path::{Path, PathBuf};

/// Live2D 模型格式（Cubism 3/4/5 共用 `.moc3` + `.model3.json`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Live2dFormat {
    /// Cubism 2（`.moc` + `model.json`），老旧格式。
    Cubism2,
    /// Cubism 3/4/5（`.moc3` + `.model3.json`）。
    Cubism3,
}

impl Live2dFormat {
    /// 转成给前端展示的字符串。
    pub fn to_str(self) -> &'static str {
        match self {
            Live2dFormat::Cubism2 => "cubism2",
            Live2dFormat::Cubism3 => "cubism3",
        }
    }
}

/// 解析后的 Live2D 配置。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLive2dConfig {
    /// 模型根目录。
    pub model_dir: PathBuf,
    /// 模型清单文件（`.model3.json` 或 `model.json`），未配置/未找到时为 `None`。
    pub model_file: Option<PathBuf>,
    /// 模型格式。
    pub format: Option<Live2dFormat>,
}

impl Default for ResolvedLive2dConfig {
    fn default() -> Self {
        Self {
            model_dir: default_model_dir(),
            model_file: None,
            format: None,
        }
    }
}

/// 用户默认 Live2D 模型目录：`~/.zapmomo/models/live2d`。
pub fn default_model_dir() -> PathBuf {
    crate::config::settings::get_models_dir().join("live2d")
}

/// 在指定目录中定位模型清单文件。
///
/// 优先顶层扫描，找不到再递归一层子目录。匹配规则：
/// - `*.model3.json` → Cubism 3/4/5
/// - `model.json` → Cubism 2
pub fn find_model_file(dir: &Path) -> Option<(PathBuf, Live2dFormat)> {
    // 顶层：优先 *.model3.json（现代格式），再 model.json（Cubism 2）
    if let Some((path, fmt)) = scan_for_model(dir) {
        return Some((path, fmt));
    }
    // 递归一层子目录
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let sub = entry.path();
            if sub.is_dir()
                && let Some((path, fmt)) = scan_for_model(&sub)
            {
                return Some((path, fmt));
            }
        }
    }
    None
}

/// 在单个目录里扫描模型清单文件（不递归）。
fn scan_for_model(dir: &Path) -> Option<(PathBuf, Live2dFormat)> {
    let mut cubism2: Option<PathBuf> = None;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name()?.to_string_lossy();
            if name.ends_with(".model3.json") {
                return Some((path, Live2dFormat::Cubism3));
            }
            if name == "model.json" && cubism2.is_none() {
                cubism2 = Some(path);
            }
        }
    }
    cubism2.map(|p| (p, Live2dFormat::Cubism2))
}

/// 解析模型目录：settings 中配置的目录（支持 `${env.VAR}` 与相对路径锚定配置目录），
/// 未配置时回退默认目录。
fn resolve_model_dir(settings: Option<&Live2dSettings>) -> Result<PathBuf, String> {
    if let Some(dir) = settings.and_then(|s| s.model_dir.as_deref()) {
        let expanded = resolve_env_ref(dir)?;
        let p = PathBuf::from(expanded);
        return Ok(if p.is_absolute() {
            p
        } else {
            crate::config::settings::get_settings_dir().join(p)
        });
    }
    Ok(default_model_dir())
}

/// 合并配置：解析模型目录并定位模型清单文件。
pub fn resolve(settings: Option<&Live2dSettings>) -> Result<ResolvedLive2dConfig, String> {
    let model_dir = resolve_model_dir(settings)?;
    let (model_file, format) = find_model_file(&model_dir)
        .map(|(p, f)| (Some(p), Some(f)))
        .unwrap_or((None, None));
    Ok(ResolvedLive2dConfig {
        model_dir,
        model_file,
        format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::run_with_temp_home;

    /// 在临时目录下创建最小 Live2D 模型骨架（仅清单文件，不校验 moc3 内容）。
    fn make_model(dir: &Path, manifest_name: &str) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let manifest = dir.join(manifest_name);
        std::fs::write(&manifest, "{}").unwrap();
        manifest
    }

    #[test]
    fn test_find_model_file_top_level_model3() {
        run_with_temp_home(|home| {
            let dir = home.join("m1");
            make_model(&dir, "火花.model3.json");
            let (path, fmt) = find_model_file(&dir).unwrap();
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with(".model3.json")
            );
            assert_eq!(fmt, Live2dFormat::Cubism3);
        });
    }

    #[test]
    fn test_find_model_file_top_level_model_json() {
        run_with_temp_home(|home| {
            let dir = home.join("m2");
            make_model(&dir, "model.json");
            let (_, fmt) = find_model_file(&dir).unwrap();
            assert_eq!(fmt, Live2dFormat::Cubism2);
        });
    }

    #[test]
    fn test_find_model_file_prefers_model3() {
        run_with_temp_home(|home| {
            let dir = home.join("m3");
            make_model(&dir, "model.json");
            make_model(&dir, "a.model3.json");
            let (_, fmt) = find_model_file(&dir).unwrap();
            assert_eq!(fmt, Live2dFormat::Cubism3);
        });
    }

    #[test]
    fn test_find_model_file_recurses_one_level() {
        run_with_temp_home(|home| {
            let dir = home.join("outer");
            let sub = dir.join("inner");
            make_model(&sub, "m.model3.json");
            let (path, fmt) = find_model_file(&dir).unwrap();
            assert_eq!(fmt, Live2dFormat::Cubism3);
            assert_eq!(path, sub.join("m.model3.json"));
        });
    }

    #[test]
    fn test_find_model_file_missing() {
        run_with_temp_home(|home| {
            let dir = home.join("empty");
            std::fs::create_dir_all(&dir).unwrap();
            assert!(find_model_file(&dir).is_none());
        });
    }

    #[test]
    fn test_resolve_default_dir() {
        run_with_temp_home(|home| {
            let cfg = resolve(None).unwrap();
            assert_eq!(cfg.model_dir, home.join(".zapmomo/models/live2d"));
            assert!(cfg.model_file.is_none());
        });
    }

    #[test]
    fn test_format_to_str() {
        assert_eq!(Live2dFormat::Cubism2.to_str(), "cubism2");
        assert_eq!(Live2dFormat::Cubism3.to_str(), "cubism3");
    }
}
