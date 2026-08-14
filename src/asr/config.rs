/// ASR 配置解析与校验。
///
/// 负责把 `settings.toml` 的 `[asr]` 表与 CLI flag 合并成一份已展开、已填默认值的
/// `ResolvedAsrConfig`。优先级：CLI `--model-dir` > settings > 内置默认。
use crate::config::settings::{AsrSettings, resolve_env_ref};
use std::path::{Path, PathBuf};

/// 模型包内默认文件名。
///
/// 采用 sherpa-onnx 官方 int8 配方：int8 encoder/joiner 搭配 fp32 decoder
/// （decoder 体积极小，官方 int8 示例即用 fp32 decoder）。
pub const DEFAULT_ENCODER: &str = "encoder-epoch-99-avg-1.int8.onnx";
pub const DEFAULT_DECODER: &str = "decoder-epoch-99-avg-1.onnx";
pub const DEFAULT_JOINER: &str = "joiner-epoch-99-avg-1.int8.onnx";
pub const DEFAULT_TOKENS: &str = "tokens.txt";

/// 模型安装完成所需的文件（相对目标目录）。
pub const REQUIRED_FILES: [&str; 4] = [
    DEFAULT_ENCODER,
    DEFAULT_DECODER,
    DEFAULT_JOINER,
    DEFAULT_TOKENS,
];

/// 标点模型包内文件名（CT Transformer 单文件）。
pub const DEFAULT_PUNCT_MODEL: &str = "model.onnx";

/// 标点模型安装完成所需的文件（相对目标目录）。
pub const PUNCT_REQUIRED_FILES: [&str; 1] = [DEFAULT_PUNCT_MODEL];

/// 解析后的完整 ASR 配置。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedAsrConfig {
    pub model_dir: PathBuf,
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
    pub provider: String,
    pub num_threads: i32,
    /// 每次喂给模型的采样数（@sample_rate）
    pub chunk_size: usize,
    pub sample_rate: i32,
    /// 解码方式：greedy_search | modified_beam_search
    pub decoding_method: String,
    /// 端点检测（静音自动断句）
    pub enable_endpoint: bool,
    pub rule1_min_trailing_silence: f32,
    pub rule2_min_trailing_silence: f32,
    pub rule3_min_utterance_length: f32,
    /// transducer 空白符惩罚（通常 0.0）
    pub blank_penalty: f32,
    /// 热词（空格分隔，中文直接写），提升专有名词识别
    pub hotwords: Option<String>,
    /// 是否对最终结果自动加标点
    pub enable_punctuation: bool,
    /// 标点模型 onnx 路径
    pub punctuation_model: PathBuf,
    pub debug: bool,
}

impl Default for ResolvedAsrConfig {
    fn default() -> Self {
        let model_dir = default_model_dir();
        let join = |name: &str| model_dir.join(name);
        Self {
            encoder: join(DEFAULT_ENCODER),
            decoder: join(DEFAULT_DECODER),
            joiner: join(DEFAULT_JOINER),
            tokens: join(DEFAULT_TOKENS),
            model_dir,
            provider: "cpu".to_string(),
            num_threads: 2,
            chunk_size: 3200,
            sample_rate: 16000,
            decoding_method: "greedy_search".to_string(),
            enable_endpoint: true,
            rule1_min_trailing_silence: 2.4,
            rule2_min_trailing_silence: 1.2,
            rule3_min_utterance_length: 20.0,
            blank_penalty: 0.0,
            hotwords: None,
            enable_punctuation: true,
            punctuation_model: crate::kws::model::punctuation_user_model_dir()
                .join(DEFAULT_PUNCT_MODEL),
            debug: false,
        }
    }
}

/// 用户默认模型目录：`~/.zapmomo/models/<模型名>`
pub fn user_default_model_dir() -> PathBuf {
    crate::kws::model::asr_user_model_dir()
}

/// 源码仓库中的模型目录（开发者 `./models/<模型名>`，仅作开发回退）。
fn repo_models_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join(&crate::kws::model::asr_asset().name)
}

/// 默认模型目录选择：用户已安装 > 源码仓库已下载（开发便利）> 用户默认。
///
/// 纯决策函数（不访问真实文件系统），便于测试注入路径。
fn choose_default_model_dir(user: &Path, repo: &Path) -> PathBuf {
    if user.join(DEFAULT_TOKENS).is_file() {
        user.to_path_buf()
    } else if repo.join(DEFAULT_TOKENS).is_file() {
        repo.to_path_buf()
    } else {
        user.to_path_buf()
    }
}

/// 默认模型目录（运行时解析：优先用户目录，源码开发时回退到仓库 `./models/`）。
pub fn default_model_dir() -> PathBuf {
    choose_default_model_dir(&user_default_model_dir(), &repo_models_dir())
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
    settings: Option<&AsrSettings>,
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
            crate::config::settings::get_settings_dir().join(p)
        });
    }
    Ok(default_model_dir())
}

/// 合并配置并填充默认值。
pub fn resolve(
    settings: Option<&AsrSettings>,
    cli_model_dir: Option<&Path>,
) -> Result<ResolvedAsrConfig, String> {
    let mut cfg = ResolvedAsrConfig {
        model_dir: resolve_model_dir(settings, cli_model_dir)?,
        ..ResolvedAsrConfig::default()
    };

    let s = settings;
    let file = |field: &str, default_name: &str| {
        let value = match field {
            "encoder" => s.and_then(|s| s.encoder.as_deref()),
            "decoder" => s.and_then(|s| s.decoder.as_deref()),
            "joiner" => s.and_then(|s| s.joiner.as_deref()),
            "tokens" => s.and_then(|s| s.tokens.as_deref()),
            _ => None,
        };
        resolve_file(value, default_name, &cfg.model_dir)
    };

    cfg.encoder = file("encoder", DEFAULT_ENCODER)?;
    cfg.decoder = file("decoder", DEFAULT_DECODER)?;
    cfg.joiner = file("joiner", DEFAULT_JOINER)?;
    cfg.tokens = file("tokens", DEFAULT_TOKENS)?;

    cfg.provider = s
        .and_then(|s| s.provider.clone())
        .unwrap_or_else(|| "cpu".to_string());
    cfg.num_threads = s.and_then(|s| s.num_threads).unwrap_or(2);
    cfg.chunk_size = s.and_then(|s| s.chunk_size).unwrap_or(3200);
    cfg.sample_rate = s.and_then(|s| s.sample_rate).unwrap_or(16000);
    cfg.decoding_method = s
        .and_then(|s| s.decoding_method.clone())
        .unwrap_or_else(|| "greedy_search".to_string());
    cfg.enable_endpoint = s.and_then(|s| s.enable_endpoint).unwrap_or(true);
    cfg.rule1_min_trailing_silence = s.and_then(|s| s.rule1_min_trailing_silence).unwrap_or(2.4);
    cfg.rule2_min_trailing_silence = s.and_then(|s| s.rule2_min_trailing_silence).unwrap_or(1.2);
    cfg.rule3_min_utterance_length = s.and_then(|s| s.rule3_min_utterance_length).unwrap_or(20.0);
    cfg.blank_penalty = s.and_then(|s| s.blank_penalty).unwrap_or(0.0);
    cfg.hotwords = s.and_then(|s| s.hotwords.clone());
    cfg.enable_punctuation = s.and_then(|s| s.enable_punctuation).unwrap_or(true);
    cfg.punctuation_model = match s.and_then(|s| s.punctuation_model.as_deref()) {
        Some(v) => {
            let expanded = resolve_env_ref(v)?;
            let p = PathBuf::from(&expanded);
            if p.is_absolute() {
                p
            } else {
                // 相对路径锚定到标点模型目录
                crate::kws::model::punctuation_user_model_dir().join(p)
            }
        }
        None => crate::kws::model::punctuation_user_model_dir().join(DEFAULT_PUNCT_MODEL),
    };
    cfg.debug = s.and_then(|s| s.debug).unwrap_or(false);

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::AsrSettings;
    use crate::test_util::run_with_temp_home;

    #[test]
    fn test_default_config_points_to_default_model_dir() {
        let cfg = ResolvedAsrConfig::default();
        assert_eq!(
            cfg.model_dir
                .file_name()
                .map(|s| s.to_string_lossy().to_string()),
            Some(crate::kws::model::asr_asset().name.clone())
        );
        assert_eq!(cfg.encoder.file_name().unwrap(), DEFAULT_ENCODER);
        assert_eq!(cfg.decoder.file_name().unwrap(), DEFAULT_DECODER);
        assert_eq!(cfg.joiner.file_name().unwrap(), DEFAULT_JOINER);
        assert_eq!(cfg.tokens.file_name().unwrap(), DEFAULT_TOKENS);
        assert_eq!(cfg.sample_rate, 16000);
        assert_eq!(cfg.chunk_size, 3200);
        assert!(cfg.enable_endpoint);
        assert_eq!(cfg.decoding_method, "greedy_search");
    }

    #[test]
    fn test_user_default_model_dir() {
        run_with_temp_home(|home| {
            let dir = super::user_default_model_dir();
            assert_eq!(
                dir,
                home.join(".zapmomo/models")
                    .join(crate::kws::model::asr_asset().name.as_str())
            );
        });
    }

    #[test]
    fn test_choose_default_model_dir_priority() {
        let base = tempfile::tempdir().unwrap();
        let user = base.path().join("user-model");
        let repo = base.path().join("repo-model");

        assert_eq!(choose_default_model_dir(&user, &repo), user);

        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join(DEFAULT_TOKENS), b"t").unwrap();
        assert_eq!(choose_default_model_dir(&user, &repo), repo);

        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join(DEFAULT_TOKENS), b"t").unwrap();
        assert_eq!(choose_default_model_dir(&user, &repo), user);
    }

    #[test]
    fn test_resolve_relative_model_dir_anchored_to_user_dir() {
        run_with_temp_home(|home| {
            let settings = AsrSettings {
                model_dir: Some("models/my-asr".to_string()),
                ..AsrSettings::default()
            };
            let cfg = resolve(Some(&settings), None).unwrap();
            assert_eq!(cfg.model_dir, home.join(".zapmomo/models/my-asr"));
        });
    }

    #[test]
    fn test_resolve_no_settings_uses_defaults() {
        let cfg = resolve(None, None).unwrap();
        assert_eq!(cfg, ResolvedAsrConfig::default());
    }

    fn abs_path(rel: &str) -> PathBuf {
        std::path::absolute(rel).unwrap()
    }

    #[test]
    fn test_resolve_cli_model_dir_overrides_settings() {
        let settings = AsrSettings {
            model_dir: Some("settings-model".to_string()),
            ..AsrSettings::default()
        };
        let cli = abs_path("tmp/cli-asr");
        let cfg = resolve(Some(&settings), Some(&cli)).unwrap();
        assert_eq!(cfg.model_dir, cli);
        assert_eq!(cfg.encoder.parent().unwrap(), cli);
    }

    #[test]
    fn test_resolve_settings_model_dir() {
        let dir = abs_path("opt/asr");
        let settings = AsrSettings {
            model_dir: Some(dir.to_string_lossy().to_string()),
            ..AsrSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.model_dir, dir);
        assert_eq!(cfg.encoder, dir.join(DEFAULT_ENCODER));
        assert_eq!(cfg.decoder, dir.join(DEFAULT_DECODER));
        assert_eq!(cfg.joiner, dir.join(DEFAULT_JOINER));
    }

    #[test]
    fn test_resolve_numeric_overrides() {
        let settings = AsrSettings {
            num_threads: Some(4),
            chunk_size: Some(1600),
            enable_endpoint: Some(false),
            rule1_min_trailing_silence: Some(3.0),
            ..AsrSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.num_threads, 4);
        assert_eq!(cfg.chunk_size, 1600);
        assert!(!cfg.enable_endpoint);
        assert_eq!(cfg.rule1_min_trailing_silence, 3.0);
    }

    #[test]
    fn test_default_punctuation_and_hotwords() {
        let cfg = ResolvedAsrConfig::default();
        assert_eq!(cfg.hotwords, None);
        assert!(cfg.enable_punctuation);
        assert_eq!(
            cfg.punctuation_model,
            crate::kws::model::punctuation_user_model_dir().join(DEFAULT_PUNCT_MODEL)
        );
    }

    #[test]
    fn test_resolve_hotwords_and_punctuation_overrides() {
        let settings = AsrSettings {
            hotwords: Some("你好小智 文森特卡索".to_string()),
            enable_punctuation: Some(false),
            ..AsrSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.hotwords, Some("你好小智 文森特卡索".to_string()));
        assert!(!cfg.enable_punctuation);
    }
}
