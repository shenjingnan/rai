/// LLM 配置解析：把可缺省的 `LlmSettings` 合并成解析后的 `ResolvedLlmConfig`。
///
/// 优先级与 KWS/ASR/TTS 一致：CLI 覆盖 > settings.toml > 内置默认。
/// 模型路径**不硬编码**：用户可配置任意 GGUF 路径；默认值仅作为推荐提示。
use std::path::{Path, PathBuf};

use crate::config::settings::{LlmSettings, get_models_dir, resolve_env_ref};
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
}

/// 内置默认 system prompt（用户可在 settings 覆盖）。
pub fn default_system_prompt() -> String {
    "你是 ZapMomo，一个友好的桌面 AI 伙伴。请用简洁自然的中文回答，语气亲切，不要啰嗦。".to_string()
}

/// 默认模型路径（推荐提示，指向 `~/.zapmomo/models/<模型名>/<文件名>`）。
pub fn default_model_path() -> PathBuf {
    get_models_dir()
        .join(DEFAULT_MODEL_NAME)
        .join(DEFAULT_MODEL_FILE)
}

/// 扫描 `~/.zapmomo/models/` 目录（含一层子目录），发现第一个 `.gguf` 文件。
///
/// 用于默认推荐模型路径不存在时，自动发现用户已下载的任意 GGUF 模型，
/// 让「下载模型 → 直接使用」无需手动配置路径。
fn discover_gguf() -> Option<PathBuf> {
    let dir = get_models_dir();
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
fn resolve_model_path(value: &str) -> Result<PathBuf, String> {
    let resolved = resolve_env_ref(value)?;
    let path = PathBuf::from(resolved);
    Ok(if path.is_absolute() {
        path
    } else {
        get_models_dir().join(path)
    })
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
        assert_eq!(cfg.model_path, PathBuf::from("/tmp/a.gguf"));
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
