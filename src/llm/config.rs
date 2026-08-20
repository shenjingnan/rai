/// LLM 配置解析：把可缺省的 `LlmSettings` 合并成解析后的 `ResolvedLlmConfig`。
///
/// 优先级与 KWS/ASR/TTS 一致：CLI 覆盖 > settings.toml > 内置默认。
/// 模型路径**不硬编码**：用户可配置任意 GGUF 路径；默认值仅作为推荐提示。
use std::path::{Path, PathBuf};

use crate::config::settings::{LlmSettings, get_models_dir, legacy_models_dir, resolve_env_ref};
use crate::llm::types::GenParams;

/// 默认推荐模型（仅作提示，非唯一允许的模型）。
pub const DEFAULT_MODEL_NAME: &str = "Qwen3-4B-Instruct-2507";
/// 默认推荐量化文件名。
pub const DEFAULT_MODEL_FILE: &str = "Qwen3-4B-Instruct-2507-Q4_K_M.gguf";

/// 解析后的 LLM 配置（字段全部为具体类型，非 `Option`）。
#[derive(Debug, Clone)]
pub struct ResolvedLlmConfig {
    /// 是否启用 Local LLM
    pub enabled: bool,
    /// provider 标识（第一版仅 "local"）
    pub provider: String,
    /// GGUF 模型文件绝对路径
    pub model_path: PathBuf,
    /// 角色 system prompt
    pub system_prompt: String,
    /// 采样/生成参数
    pub params: GenParams,
    /// 是否在应用启动时自动加载模型
    pub auto_load: bool,
    /// HTTP provider 的 base URL（None 表示未配置，local provider 不使用）
    pub base_url: Option<String>,
    /// HTTP provider 的 API key
    pub api_key: Option<String>,
    /// HTTP provider 的模型名
    pub model: Option<String>,
}

/// 内置默认 system prompt（用户可在 settings 覆盖）。
pub fn default_system_prompt() -> String {
    "你是 ZapMomo，一个友好的桌面 AI 伙伴。请用简洁自然的中文回答，语气亲切，不要啰嗦。".to_string()
}

/// 默认模型路径（推荐提示，指向 `~/.zapmomo/models/<模型名>/<文件名>`）。
///
/// 优先主根（新根）；data_dir 切换后主根尚无而旧根有存量时，回退旧根路径
/// （存量模型无需迁移即可被 resolve 到）。
pub fn default_model_path() -> PathBuf {
    let new = get_models_dir()
        .join(DEFAULT_MODEL_NAME)
        .join(DEFAULT_MODEL_FILE);
    if new.is_file() {
        return new;
    }
    if let Some(legacy) = legacy_models_dir() {
        let legacy_path = legacy.join(DEFAULT_MODEL_NAME).join(DEFAULT_MODEL_FILE);
        if legacy_path.is_file() {
            return legacy_path;
        }
    }
    new
}

/// 扫描模型根目录（主根 + 旧根，含一层子目录），发现第一个 `.gguf` 文件。
///
/// 用于默认推荐模型路径不存在时，自动发现用户已下载的任意 GGUF 模型，
/// 让「下载模型 → 直接使用」无需手动配置路径。双根都扫，旧根存量同样被发现。
fn discover_gguf() -> Option<PathBuf> {
    let mut dirs = vec![get_models_dir()];
    if let Some(legacy) = legacy_models_dir()
        && !dirs.contains(&legacy)
    {
        dirs.push(legacy);
    }
    for dir in dirs {
        if let Some(found) = discover_gguf_in(&dir) {
            return Some(found);
        }
    }
    None
}

fn discover_gguf_in(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Ok(sub) = std::fs::read_dir(&path) {
                for f in sub.flatten() {
                    let p = f.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("gguf") {
                        return Some(p);
                    }
                }
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
            return Some(path);
        }
    }
    None
}

/// 默认 CPU 线程数：物理核数 - 2（给系统与 ASR/TTS 留余量），最少 1。
fn default_threads() -> i32 {
    let n = std::thread::available_parallelism()
        .map(|v| v.get())
        .unwrap_or(4);
    n.saturating_sub(2).max(1) as i32
}

/// 解析 `model_path`：展开 `${env.VAR}`，相对路径锚定 `~/.zapmomo/models`。
///
/// data_dir 切换后主根尚无而旧根有存量时，相对路径回退锚定旧根（存量兼容）。
fn resolve_model_path(value: &str) -> Result<PathBuf, String> {
    let resolved = resolve_env_ref(value)?;
    let path = PathBuf::from(resolved);
    if path.is_absolute() {
        return Ok(path);
    }
    let new = get_models_dir().join(&path);
    if new.is_file() {
        return Ok(new);
    }
    if let Some(legacy) = legacy_models_dir() {
        let legacy_path = legacy.join(&path);
        if legacy_path.is_file() {
            return Ok(legacy_path);
        }
    }
    Ok(new)
}

/// 合并 settings 与 CLI 覆盖得到最终配置。
///
/// `cli_model_path` 来自 `--model-path`；`None` 时回退 settings，再回退默认推荐路径。
pub fn resolve(
    settings: Option<&LlmSettings>,
    cli_model_path: Option<&Path>,
) -> Result<ResolvedLlmConfig, String> {
    let model_path = if let Some(p) = cli_model_path {
        p.to_path_buf()
    } else if let Some(v) = settings.and_then(|s| s.model_path.as_deref()) {
        resolve_model_path(v)?
    } else {
        let default = default_model_path();
        if default.is_file() {
            default
        } else {
            discover_gguf().unwrap_or(default)
        }
    };

    let defaults = GenParams::default();

    Ok(ResolvedLlmConfig {
        enabled: settings.and_then(|s| s.enabled).unwrap_or(false),
        provider: settings
            .and_then(|s| s.provider.clone())
            .unwrap_or_else(|| "local".to_string()),
        model_path,
        system_prompt: settings
            .and_then(|s| s.system_prompt.clone())
            .unwrap_or_else(default_system_prompt),
        params: GenParams {
            context_size: settings
                .and_then(|s| s.context_size)
                .unwrap_or(defaults.context_size),
            batch_size: settings
                .and_then(|s| s.batch_size)
                .unwrap_or(defaults.batch_size),
            max_tokens: settings
                .and_then(|s| s.max_tokens)
                .unwrap_or(defaults.max_tokens),
            temperature: settings
                .and_then(|s| s.temperature)
                .unwrap_or(defaults.temperature),
            top_p: settings.and_then(|s| s.top_p).unwrap_or(defaults.top_p),
            top_k: settings.and_then(|s| s.top_k).unwrap_or(defaults.top_k),
            min_p: settings.and_then(|s| s.min_p).unwrap_or(defaults.min_p),
            repeat_penalty: settings
                .and_then(|s| s.repeat_penalty)
                .unwrap_or(defaults.repeat_penalty),
            seed: settings.and_then(|s| s.seed).unwrap_or(defaults.seed),
            threads: settings
                .and_then(|s| s.threads)
                .unwrap_or_else(default_threads),
            gpu_layers: settings
                .and_then(|s| s.gpu_layers)
                .unwrap_or(defaults.gpu_layers),
            enable_thinking: settings
                .and_then(|s| s.enable_thinking)
                .unwrap_or(defaults.enable_thinking),
        },
        auto_load: settings.and_then(|s| s.auto_load).unwrap_or(false),
        base_url: settings.and_then(|s| s.base_url.clone()),
        api_key: settings.and_then(|s| s.api_key.clone()),
        model: settings.and_then(|s| s.model.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_resolve_points_to_recommended_model() {
        crate::test_util::run_with_temp_home(|_| {
            let cfg = resolve(None, None).unwrap();
            assert!(!cfg.enabled);
            assert_eq!(cfg.provider, "local");
            // 临时 HOME 下无已下载模型，回退到推荐默认路径
            assert!(cfg.model_path.ends_with(DEFAULT_MODEL_FILE));
            assert_eq!(cfg.params.context_size, 8192);
            assert_eq!(cfg.params.gpu_layers, 0);
        });
    }

    #[test]
    fn test_default_path_dual_root_fallback() {
        crate::test_util::run_with_temp_home(|home| {
            crate::test_util::set_custom_data_dir(home);
            let legacy_models = home.join(".zapmomo/models");
            // 旧根有推荐默认模型 → resolve 落旧根
            let legacy_default = legacy_models
                .join(DEFAULT_MODEL_NAME)
                .join(DEFAULT_MODEL_FILE);
            std::fs::create_dir_all(legacy_default.parent().unwrap()).unwrap();
            std::fs::write(&legacy_default, b"GGUF").unwrap();
            let cfg = resolve(None, None).unwrap();
            assert_eq!(cfg.model_path, legacy_default);

            // 默认不在 → discover 扫旧根找到其它 gguf
            std::fs::remove_file(&legacy_default).unwrap();
            let other = legacy_models.join("my-model/x.gguf");
            std::fs::create_dir_all(other.parent().unwrap()).unwrap();
            std::fs::write(&other, b"G").unwrap();
            let cfg2 = resolve(None, None).unwrap();
            assert_eq!(cfg2.model_path, other);
        });
    }

    #[test]
    fn test_resolve_relative_model_path_legacy_fallback() {
        crate::test_util::run_with_temp_home(|home| {
            crate::test_util::set_custom_data_dir(home);
            // 相对路径在旧根命中 → 锚定旧根（存量兼容）
            let p = home.join(".zapmomo/models/rel/model.gguf");
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"G").unwrap();
            let settings = LlmSettings {
                model_path: Some("rel/model.gguf".into()),
                ..Default::default()
            };
            let cfg = resolve(Some(&settings), None).unwrap();
            assert_eq!(cfg.model_path, p);
        });
    }

    #[test]
    fn test_resolve_discovers_downloaded_gguf() {
        crate::test_util::run_with_temp_home(|home| {
            let models = home.join(".zapmomo/models/some-model");
            std::fs::create_dir_all(&models).unwrap();
            std::fs::write(models.join("model.gguf"), b"GGUF").unwrap();
            let cfg = resolve(None, None).unwrap();
            // 默认推荐路径不存在时，自动发现已下载的 GGUF
            assert!(cfg.model_path.ends_with("model.gguf"));
        });
    }

    #[test]
    fn test_cli_model_path_overrides() {
        let cfg = resolve(None, Some(Path::new("/tmp/custom.gguf"))).unwrap();
        assert_eq!(cfg.model_path, PathBuf::from("/tmp/custom.gguf"));
    }

    #[test]
    fn test_settings_enabled_and_params() {
        let s = LlmSettings {
            enabled: Some(true),
            model_path: Some("/tmp/a.gguf".to_string()),
            temperature: Some(0.9),
            max_tokens: Some(128),
            ..Default::default()
        };
        let cfg = resolve(Some(&s), None).unwrap();
        assert!(cfg.enabled);
        // `/tmp/a.gguf` 在 Windows 上不是绝对路径，会被锚定到 models 目录；这里只断言文件名，
        // 避免依赖平台的绝对路径判定。
        assert_eq!(cfg.model_path.file_name().unwrap(), "a.gguf");
        assert_eq!(cfg.params.temperature, 0.9);
        assert_eq!(cfg.params.max_tokens, 128);
        // 未配置项回退默认
        assert_eq!(cfg.params.context_size, 8192);
    }

    #[test]
    fn test_default_threads_is_positive() {
        assert!(default_threads() >= 1);
    }
}
