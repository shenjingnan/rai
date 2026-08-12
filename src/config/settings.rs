/// Settings - TOML 配置管理
///
/// 提供通用的配置读写功能，支持 ${env.VAR} 环境变量引用。
/// 配置文件存储在 `~/.{{project_name}}/settings.toml`。
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PROJECT_DIR: &str = ".ai-rust-starter";
const SETTINGS_FILE: &str = "settings.toml";

/// 获取用户 home 目录（跨平台：macOS/Linux 用 $HOME，Windows 用 %USERPROFILE%）
pub fn get_home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
        .into()
}

/// 获取配置目录路径
pub fn get_settings_dir() -> PathBuf {
    get_home_dir().join(PROJECT_DIR)
}

/// 获取设置文件路径
pub fn get_settings_path() -> PathBuf {
    get_settings_dir().join(SETTINGS_FILE)
}

/// 解析 ${env.VAR} 引用
///
/// - "${env.MY_VAR}" → 从环境变量 MY_VAR 读取
/// - "plain-value" → 原样返回
pub fn resolve_env_ref(value: &str) -> Result<String, String> {
    if let Some(captures) = value
        .strip_prefix("${env.")
        .and_then(|s| s.strip_suffix('}'))
    {
        let env_var = captures;
        if env_var.is_empty() {
            return Err("环境变量名称为空".to_string());
        }
        match std::env::var(env_var) {
            Ok(resolved) => Ok(resolved),
            Err(_) => Err(format!(
                "环境变量 {env_var} 未设置。请在 {SETTINGS_FILE} 中配置或设置环境变量 {env_var}。"
            )),
        }
    } else {
        Ok(value.to_string())
    }
}

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// 调试模式
    #[serde(default)]
    pub debug: bool,
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// 自定义配置项（示例）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<std::collections::HashMap<String, String>>,
    /// 唤醒词检测（KWS）配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kws: Option<KwsSettings>,
}

/// 唤醒词检测（KWS）配置。
///
/// 全部字段可缺省：未配置的项在解析时回退到 `kws::config` 的内置默认值，
/// 因此这里用 `Option` 以区分「未配置」与「配置了」。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct KwsSettings {
    /// 模型目录（支持 ${env.VAR} 引用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<String>,
    /// encoder onnx 文件名（缺省用模型目录下 chunk-16 变体）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
    /// decoder onnx 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<String>,
    /// joiner onnx 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joiner: Option<String>,
    /// tokens.txt 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    /// 关键词文件路径（缺省 = <model_dir>/test_wavs/keywords.txt）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords_file: Option<String>,
    /// 推理后端，缺省 "cpu"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// 推理线程数，缺省 2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_threads: Option<i32>,
    /// 每次喂给模型的采样数（@16k），缺省 3200（0.2s）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    /// 模型输入采样率，缺省 16000
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<i32>,
    /// 关键词 boosting 分数，缺省 1.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords_score: Option<f32>,
    /// 触发阈值，缺省 0.25（越大越不容易误触发）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords_threshold: Option<f32>,
    /// 调试输出，缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            debug: false,
            log_level: default_log_level(),
            custom: None,
            kws: None,
        }
    }
}

/// 加载 ~/.ai-rust-starter/settings.toml
///
/// 文件不存在时返回 None，不报错。
pub fn load_settings() -> Result<Option<AppConfig>, String> {
    let file_path = get_settings_path();

    let content = match std::fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };

    let config: AppConfig = toml::from_str(&content).map_err(|e| format!("TOML 格式错误: {e}"))?;

    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::run_with_temp_home;

    fn write_toml_settings(home: &std::path::Path, content: &str) {
        let settings_dir = home.join(PROJECT_DIR);
        std::fs::create_dir_all(&settings_dir).unwrap();
        std::fs::write(settings_dir.join(SETTINGS_FILE), content).unwrap();
    }

    #[test]
    fn test_get_settings_path() {
        run_with_temp_home(|home| {
            let path = get_settings_path();
            assert_eq!(path, home.join(".ai-rust-starter/settings.toml"));
        });
    }

    #[test]
    fn test_get_settings_dir() {
        run_with_temp_home(|home| {
            let dir = get_settings_dir();
            assert_eq!(dir, home.join(".ai-rust-starter"));
        });
    }

    #[test]
    fn test_resolve_env_ref_plain_value() {
        assert_eq!(resolve_env_ref("plain-value").unwrap(), "plain-value");
        assert_eq!(
            resolve_env_ref("https://example.com").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn test_resolve_env_ref_from_env() {
        unsafe {
            std::env::set_var("TEST_MY_VAR", "test-resolved-value");
        }
        assert_eq!(
            resolve_env_ref("${env.TEST_MY_VAR}").unwrap(),
            "test-resolved-value"
        );
        unsafe {
            std::env::remove_var("TEST_MY_VAR");
        }
    }

    #[test]
    fn test_resolve_env_ref_missing_var() {
        let result = resolve_env_ref("${env.NONEXISTENT_VAR_XYZ}");
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("NONEXISTENT_VAR_XYZ"));
    }

    #[test]
    fn test_resolve_env_ref_empty() {
        assert_eq!(resolve_env_ref("").unwrap(), "");
    }

    #[test]
    fn test_resolve_env_ref_empty_env_var_name() {
        let result = resolve_env_ref("${env.}");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_settings_file_not_found() {
        run_with_temp_home(|_| {
            let result = load_settings().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn test_load_settings_invalid_toml() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "{invalid}");
            let result = load_settings();
            assert!(result.is_err());
            assert!(result.err().unwrap().contains("TOML 格式错误"));
        });
    }

    #[test]
    fn test_load_settings_empty() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "");
            let result = load_settings().unwrap().unwrap();
            assert!(!result.debug);
            assert_eq!(result.log_level, "info");
            assert!(result.custom.is_none());
        });
    }

    #[test]
    fn test_load_settings_full() {
        run_with_temp_home(|home| {
            write_toml_settings(
                home,
                "debug = true\nlog_level = \"debug\"\n\n[custom]\nkey1 = \"value1\"\n",
            );
            let result = load_settings().unwrap().unwrap();
            assert!(result.debug);
            assert_eq!(result.log_level, "debug");
            assert_eq!(result.custom.unwrap().get("key1").unwrap(), "value1");
        });
    }

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(!config.debug);
        assert_eq!(config.log_level, "info");
        assert!(config.custom.is_none());
    }

    #[test]
    fn test_app_config_serde_roundtrip() {
        let config = AppConfig {
            debug: true,
            log_level: "warn".to_string(),
            custom: Some(std::collections::HashMap::new()),
            kws: None,
        };
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_load_settings_with_kws_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[kws]\nnum_threads = 4\nchunk_size = 1600\n");
            let result = load_settings().unwrap().unwrap();
            let kws = result.kws.unwrap();
            assert_eq!(kws.num_threads, Some(4));
            assert_eq!(kws.chunk_size, Some(1600));
            // 未配置的字段保持 None
            assert_eq!(kws.model_dir, None);
            assert_eq!(kws.keywords_threshold, None);
        });
    }

    #[test]
    fn test_load_settings_without_kws_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "debug = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(result.kws.is_none());
        });
    }

    #[test]
    fn test_kws_settings_serde_roundtrip() {
        let kws = KwsSettings {
            model_dir: Some("${env.KWS_MODEL_DIR}".to_string()),
            encoder: Some("encoder.onnx".to_string()),
            decoder: None,
            joiner: None,
            tokens: None,
            keywords_file: Some("kw.txt".to_string()),
            provider: Some("cpu".to_string()),
            num_threads: Some(4),
            chunk_size: Some(1600),
            sample_rate: Some(16000),
            keywords_score: Some(1.0),
            keywords_threshold: Some(0.3),
            debug: Some(false),
        };
        let toml_str = toml::to_string(&kws).unwrap();
        let deserialized: KwsSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(kws, deserialized);
        // 未配置字段应被 skip_serializing_if 忽略
        assert!(!toml_str.contains("decoder"));
    }

    #[test]
    fn test_kws_settings_env_ref_resolution() {
        unsafe {
            std::env::set_var("KWS_MODEL_DIR", "/tmp/kws-model");
        }
        let kws = KwsSettings {
            model_dir: Some("${env.KWS_MODEL_DIR}".to_string()),
            ..KwsSettings::default()
        };
        assert_eq!(
            resolve_env_ref(kws.model_dir.as_ref().unwrap()).unwrap(),
            "/tmp/kws-model"
        );
        unsafe {
            std::env::remove_var("KWS_MODEL_DIR");
        }
    }
}
