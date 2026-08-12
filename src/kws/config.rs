/// KWS 配置解析与校验。
///
/// 负责把 `settings.toml` 的 `[kws]` 表与 CLI flag 合并成一份已展开、已填默认值的
/// `ResolvedKwsConfig`。优先级：CLI `--model-dir` > settings > 内置默认。
use crate::config::settings::{KwsSettings, resolve_env_ref};
use std::path::{Path, PathBuf};

/// 模型包内默认文件名（chunk-16 变体，与官方测试命令一致）。
pub const DEFAULT_ENCODER: &str = "encoder-epoch-13-avg-2-chunk-16-left-64.onnx";
pub const DEFAULT_DECODER: &str = "decoder-epoch-13-avg-2-chunk-16-left-64.onnx";
pub const DEFAULT_JOINER: &str = "joiner-epoch-13-avg-2-chunk-16-left-64.onnx";
pub const DEFAULT_TOKENS: &str = "tokens.txt";
/// 模型包内自带的关键词文件（中英混合，开箱即用）。
pub const DEFAULT_KEYWORDS_REL: &str = "test_wavs/keywords.txt";

/// 解析后的完整 KWS 配置。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedKwsConfig {
    pub model_dir: PathBuf,
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
    pub keywords_file: PathBuf,
    pub provider: String,
    pub num_threads: i32,
    /// 每次喂给模型的采样数（@sample_rate）
    pub chunk_size: usize,
    pub sample_rate: i32,
    pub keywords_score: f32,
    pub keywords_threshold: f32,
    pub debug: bool,
}

impl Default for ResolvedKwsConfig {
    fn default() -> Self {
        let model_dir = default_model_dir();
        let join = |name: &str| model_dir.join(name);
        Self {
            keywords_file: join(DEFAULT_KEYWORDS_REL),
            encoder: join(DEFAULT_ENCODER),
            decoder: join(DEFAULT_DECODER),
            joiner: join(DEFAULT_JOINER),
            tokens: join(DEFAULT_TOKENS),
            model_dir,
            provider: "cpu".to_string(),
            num_threads: 2,
            chunk_size: 3200,
            sample_rate: 16000,
            keywords_score: 1.0,
            keywords_threshold: 0.25,
            debug: false,
        }
    }
}

/// 默认模型目录：`<仓库根>/models/<模型名>`，用 `CARGO_MANIFEST_DIR` 定位，
/// 不受进程当前工作目录影响。
pub fn default_model_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20")
}

/// 展开 settings 中的路径字符串（支持 `${env.VAR}`），未配置时用默认文件名。
/// 返回的路径若为相对路径则拼接在 `model_dir` 下。
fn resolve_file(
    settings_value: Option<&str>,
    default_name: &str,
    model_dir: &Path,
) -> Result<PathBuf, String> {
    match settings_value {
        Some(v) => {
            let expanded = resolve_env_ref(v)?;
            let p = PathBuf::from(&expanded);
            Ok(if p.is_absolute() {
                p
            } else {
                model_dir.join(p)
            })
        }
        None => Ok(model_dir.join(default_name)),
    }
}

/// 解析模型目录：CLI > settings > 默认。
fn resolve_model_dir(
    settings: Option<&KwsSettings>,
    cli_model_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(dir) = cli_model_dir {
        return Ok(dir.to_path_buf());
    }
    if let Some(dir) = settings.and_then(|s| s.model_dir.as_deref()) {
        let expanded = resolve_env_ref(dir)?;
        let p = PathBuf::from(expanded);
        return Ok(if p.is_absolute() {
            p
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(p)
        });
    }
    Ok(default_model_dir())
}

/// 合并配置并填充默认值。
pub fn resolve(
    settings: Option<&KwsSettings>,
    cli_model_dir: Option<&Path>,
) -> Result<ResolvedKwsConfig, String> {
    let mut cfg = ResolvedKwsConfig {
        model_dir: resolve_model_dir(settings, cli_model_dir)?,
        ..ResolvedKwsConfig::default()
    };

    let s = settings;
    let file = |field: &str, default_name: &str| {
        let value = match field {
            "encoder" => s.and_then(|s| s.encoder.as_deref()),
            "decoder" => s.and_then(|s| s.decoder.as_deref()),
            "joiner" => s.and_then(|s| s.joiner.as_deref()),
            "tokens" => s.and_then(|s| s.tokens.as_deref()),
            "keywords_file" => s.and_then(|s| s.keywords_file.as_deref()),
            _ => None,
        };
        resolve_file(value, default_name, &cfg.model_dir)
    };

    cfg.encoder = file("encoder", DEFAULT_ENCODER)?;
    cfg.decoder = file("decoder", DEFAULT_DECODER)?;
    cfg.joiner = file("joiner", DEFAULT_JOINER)?;
    cfg.tokens = file("tokens", DEFAULT_TOKENS)?;
    cfg.keywords_file = file("keywords_file", DEFAULT_KEYWORDS_REL)?;

    cfg.provider = s
        .and_then(|s| s.provider.clone())
        .unwrap_or_else(|| "cpu".to_string());
    cfg.num_threads = s.and_then(|s| s.num_threads).unwrap_or(2);
    cfg.chunk_size = s.and_then(|s| s.chunk_size).unwrap_or(3200);
    cfg.sample_rate = s.and_then(|s| s.sample_rate).unwrap_or(16000);
    cfg.keywords_score = s.and_then(|s| s.keywords_score).unwrap_or(1.0);
    cfg.keywords_threshold = s.and_then(|s| s.keywords_threshold).unwrap_or(0.25);
    cfg.debug = s.and_then(|s| s.debug).unwrap_or(false);

    Ok(cfg)
}

/// 解析 keywords 文件，返回显示词列表（供日志与校验）。
///
/// 每行一个：跳过空行与 `#` 注释；取 `@` 后的显示词，无 `@` 时整行作为关键词。
pub fn parse_keywords_file(path: &Path) -> Result<Vec<String>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("无法读取关键词文件 {}: {}", path.display(), e))?;
    let mut keywords = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let display = line.rsplit_once('@').map(|(_, d)| d).unwrap_or(line);
        keywords.push(display.trim().to_string());
    }
    if keywords.is_empty() {
        return Err(format!("关键词文件 {} 为空", path.display()));
    }
    Ok(keywords)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::KwsSettings;
    use std::io::Write;

    fn temp_keywords_file(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_default_config_points_to_default_model_dir() {
        let cfg = ResolvedKwsConfig::default();
        assert_eq!(
            cfg.model_dir,
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("models")
                .join("sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20")
        );
        assert_eq!(cfg.encoder.file_name().unwrap(), DEFAULT_ENCODER);
        assert_eq!(cfg.sample_rate, 16000);
        assert_eq!(cfg.chunk_size, 3200);
        assert_eq!(cfg.keywords_threshold, 0.25);
    }

    #[test]
    fn test_resolve_no_settings_uses_defaults() {
        let cfg = resolve(None, None).unwrap();
        assert_eq!(cfg, ResolvedKwsConfig::default());
    }

    /// 构造跨平台绝对路径（Windows 上 `/xxx` 无盘符不是绝对路径，避免测试依赖 POSIX 语义）
    fn abs_path(rel: &str) -> PathBuf {
        std::path::absolute(rel).unwrap()
    }

    #[test]
    fn test_resolve_cli_model_dir_overrides_settings() {
        let settings = KwsSettings {
            model_dir: Some("settings-model".to_string()),
            ..KwsSettings::default()
        };
        let cli = abs_path("tmp/cli-model");
        let cfg = resolve(Some(&settings), Some(&cli)).unwrap();
        assert_eq!(cfg.model_dir, cli);
        assert_eq!(cfg.encoder.parent().unwrap(), cli);
    }

    #[test]
    fn test_resolve_settings_model_dir() {
        let dir = abs_path("opt/kws");
        let settings = KwsSettings {
            model_dir: Some(dir.to_string_lossy().to_string()),
            ..KwsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.model_dir, dir);
        assert_eq!(cfg.encoder, dir.join(DEFAULT_ENCODER));
        assert_eq!(cfg.keywords_file, dir.join(DEFAULT_KEYWORDS_REL));
    }

    #[test]
    fn test_resolve_relative_encoder_joins_model_dir() {
        let dir = abs_path("opt/kws");
        let settings = KwsSettings {
            model_dir: Some(dir.to_string_lossy().to_string()),
            encoder: Some("my-encoder.int8.onnx".to_string()),
            ..KwsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.encoder, dir.join("my-encoder.int8.onnx"));
    }

    #[test]
    fn test_resolve_absolute_encoder_kept_as_is() {
        let dir = abs_path("opt/kws");
        let enc = abs_path("elsewhere/enc.onnx");
        let settings = KwsSettings {
            model_dir: Some(dir.to_string_lossy().to_string()),
            encoder: Some(enc.to_string_lossy().to_string()),
            ..KwsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.encoder, enc);
    }

    #[test]
    fn test_resolve_env_ref_in_model_dir() {
        let dir = abs_path("env/kws");
        unsafe {
            std::env::set_var("KWS_MODEL_DIR", dir.to_string_lossy().as_ref());
        }
        let settings = KwsSettings {
            model_dir: Some("${env.KWS_MODEL_DIR}".to_string()),
            ..KwsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.model_dir, dir);
        unsafe {
            std::env::remove_var("KWS_MODEL_DIR");
        }
    }

    #[test]
    fn test_resolve_numeric_overrides() {
        let settings = KwsSettings {
            num_threads: Some(4),
            chunk_size: Some(1600),
            keywords_threshold: Some(0.5),
            ..KwsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.num_threads, 4);
        assert_eq!(cfg.chunk_size, 1600);
        assert_eq!(cfg.keywords_threshold, 0.5);
    }

    #[test]
    fn test_parse_keywords_file_basic() {
        let f = temp_keywords_file("L AY1 T AH1 P @LIGHT_UP\nw én s ēn @文森\n");
        let kws = parse_keywords_file(f.path()).unwrap();
        assert_eq!(kws, vec!["LIGHT_UP".to_string(), "文森".to_string()]);
    }

    #[test]
    fn test_parse_keywords_file_skips_blank_and_comments() {
        let f = temp_keywords_file(
            "# 注释行\n\n  \nL AY1 T AH1 P @LIGHT_UP\n# 另一个注释\nn ǚ ér @女儿\n",
        );
        let kws = parse_keywords_file(f.path()).unwrap();
        assert_eq!(kws, vec!["LIGHT_UP".to_string(), "女儿".to_string()]);
    }

    #[test]
    fn test_parse_keywords_file_without_at_sign() {
        let f = temp_keywords_file("L AY1 T AH1 P\n");
        let kws = parse_keywords_file(f.path()).unwrap();
        assert_eq!(kws, vec!["L AY1 T AH1 P".to_string()]);
    }

    #[test]
    fn test_parse_keywords_file_empty_errors() {
        let f = temp_keywords_file("  \n# only comment\n");
        assert!(parse_keywords_file(f.path()).is_err());
    }

    #[test]
    fn test_parse_keywords_file_missing_file_errors() {
        assert!(parse_keywords_file(Path::new("/nonexistent/kw.txt")).is_err());
    }
}
