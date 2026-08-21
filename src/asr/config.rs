/// ASR 配置解析与校验。
///
/// 负责把 `settings.toml` 的 `[asr]` 表与 CLI flag 合并成一份已展开、已填默认值的
/// `ResolvedAsrConfig`。优先级：CLI `--model-dir` > settings > 内置默认。
use crate::config::settings::{AsrSettings, resolve_env_ref};
use serde::Deserialize;
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
    /// 是否启用 ASR（语音会话「能识别」的前提），缺省 false
    pub enabled: bool,
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
            enabled: false,
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
            punctuation_model: punctuation_default_dir().join(DEFAULT_PUNCT_MODEL),
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

/// 默认模型目录选择：用户已安装 > 旧默认根存量（data_dir 切换后）> 源码仓库已下载（开发便利）> 用户默认。
///
/// 纯决策函数（不访问真实文件系统），便于测试注入路径。
fn choose_default_model_dir(user: &Path, legacy: Option<&Path>, repo: &Path) -> PathBuf {
    if user.join(DEFAULT_TOKENS).is_file() {
        user.to_path_buf()
    } else if legacy.is_some_and(|l| l.join(DEFAULT_TOKENS).is_file()) {
        legacy.unwrap().to_path_buf()
    } else if repo.join(DEFAULT_TOKENS).is_file() {
        repo.to_path_buf()
    } else {
        user.to_path_buf()
    }
}

/// 默认模型目录（运行时解析：优先用户目录，旧根存量兜底，源码开发时回退到仓库 `./models/`）。
pub fn default_model_dir() -> PathBuf {
    // legacy 与 user 层次对等：旧根下对应模型的子目录（user 是 `models/<模型名>`）
    let legacy = crate::config::settings::legacy_models_dir()
        .map(|l| l.join(&crate::kws::model::asr_asset().name));
    choose_default_model_dir(
        &user_default_model_dir(),
        legacy.as_deref(),
        &repo_models_dir(),
    )
}

/// 标点模型默认目录：用户目录（`~/.zapmomo/models/<标点名>`）优先，旧根存量兜底。
fn punctuation_default_dir() -> PathBuf {
    let new = crate::kws::model::punctuation_user_model_dir();
    if new.join(DEFAULT_PUNCT_MODEL).is_file() {
        return new;
    }
    if let Some(legacy) = crate::config::settings::legacy_models_dir() {
        let legacy_dir = legacy.join(new.file_name().unwrap_or_default());
        if legacy_dir.join(DEFAULT_PUNCT_MODEL).is_file() {
            return legacy_dir;
        }
    }
    new
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

/// onnx 默认文件名探测：settings 未显式配置某 onnx 文件时按模型目录内容选择。
///
/// 与 KWS 的 `detect_default_onnx` 规则不同：ASR 官方 int8 配方是 int8 encoder/joiner +
/// fp32 decoder（int8 偏好按组件分方向），且文件名不一定含 chunk-16（双语模型就不含），
/// 故各自维护、注释互引，不做参数化共享。
///
/// 规则（确定性，read_dir 顺序不确定故候选排序）：
/// 1. 默认常量文件名存在 → 直接用（已装注册模型零行为变化，实测 6 个注册包常量文件都在）；
/// 2. 否则收集目录中 `{prefix}-` 开头、`.onnx` 结尾的文件：优先子集 =
///    `prefer_int8` 时含 `.int8`、否则不含 `.int8`，子集内字母序取第一个；
///    优先子集为空 → 全体候选字母序取第一个（如 int8-only 目录的 decoder 取 int8，可运行）；
/// 3. 目录不存在或无候选 → 回退默认常量名（后续预检报「缺少模型文件」，错误路径清晰）。
fn detect_default_onnx(
    model_dir: &Path,
    prefix: &str,
    fallback: &str,
    prefer_int8: bool,
) -> String {
    if model_dir.join(fallback).is_file() {
        return fallback.to_string();
    }
    let Ok(entries) = std::fs::read_dir(model_dir) else {
        return fallback.to_string();
    };
    let mut candidates: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.starts_with(&format!("{prefix}-")) && n.ends_with(".onnx"))
        .collect();
    candidates.sort();
    candidates
        .iter()
        .find(|n| n.contains(".int8") == prefer_int8)
        .or_else(|| candidates.first())
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

/// settings 未显式配置某文件字段时的默认名探测入口（tokens 各模型同名，不探测）。
///
/// int8 偏好按组件分方向：encoder/joiner 偏好 int8（官方量化配方，与默认常量一致），
/// decoder 偏好 fp32（体积小、int8 收益可忽略，官方示例即用 fp32）。
fn detect_default_name(field: &str, model_dir: &Path, fallback: &str) -> String {
    match field {
        "encoder" => detect_default_onnx(model_dir, "encoder", fallback, true),
        "decoder" => detect_default_onnx(model_dir, "decoder", fallback, false),
        "joiner" => detect_default_onnx(model_dir, "joiner", fallback, true),
        _ => fallback.to_string(),
    }
}

/// 目录内是否探测得到完整的一套 ASR 模型文件（模型无关，替代按默认文件名硬编码的
/// 判定，供模型库 external/HF 导入的完整性检查复用；对称 KWS 的 `kws_files_present`）。
pub fn asr_files_present(model_dir: &Path) -> bool {
    let files = [
        detect_default_onnx(model_dir, "encoder", DEFAULT_ENCODER, true),
        detect_default_onnx(model_dir, "decoder", DEFAULT_DECODER, false),
        detect_default_onnx(model_dir, "joiner", DEFAULT_JOINER, true),
        DEFAULT_TOKENS.to_string(),
    ];
    files.iter().all(|f| model_dir.join(f).is_file())
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
        // 未显式配置时按模型目录内容探测默认文件名（外部导入/手工放置的目录
        // 可能不叫默认名；显式 settings 覆盖优先，与 KWS resolve 语义一致）
        let detected = if value.is_none() {
            detect_default_name(field, &cfg.model_dir, default_name)
        } else {
            default_name.to_string()
        };
        resolve_file(value, &detected, &cfg.model_dir)
    };

    cfg.encoder = file("encoder", DEFAULT_ENCODER)?;
    cfg.decoder = file("decoder", DEFAULT_DECODER)?;
    cfg.joiner = file("joiner", DEFAULT_JOINER)?;
    cfg.tokens = file("tokens", DEFAULT_TOKENS)?;

    cfg.enabled = s.and_then(|s| s.enabled).unwrap_or(false);
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
                // 相对路径锚定到标点模型目录（含旧根兜底）
                punctuation_default_dir().join(p)
            }
        }
        None => punctuation_default_dir().join(DEFAULT_PUNCT_MODEL),
    };
    cfg.debug = s.and_then(|s| s.debug).unwrap_or(false);

    Ok(cfg)
}

/// `set_asr_params` 载荷：可调整的 ASR 引擎/运行参数（snake_case 直传，缺省项不修改）。
///
/// 与 Tauri crate 的 `KwsParamsPatch` 对称，但放在 lib crate 内以便 `cargo test` 单测。
/// 引擎参数在 `start_asr_listen` 时固化：保存后需重启识别才生效（由前端处理）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AsrParamsPatch {
    pub num_threads: Option<i32>,
    pub chunk_size: Option<usize>,
    pub enable_endpoint: Option<bool>,
    pub rule1_min_trailing_silence: Option<f32>,
    pub rule2_min_trailing_silence: Option<f32>,
    pub rule3_min_utterance_length: Option<f32>,
    pub blank_penalty: Option<f32>,
    pub hotwords: Option<String>,
    pub enable_punctuation: Option<bool>,
    pub debug: Option<bool>,
}

impl AsrParamsPatch {
    /// 先整体校验（任一越界立即 Err），再逐项写入 `AsrSettings`，保证出错时不部分修改。
    pub fn apply_to(&self, asr: &mut AsrSettings) -> Result<(), String> {
        if let Some(v) = self.num_threads
            && !(1..=32).contains(&v)
        {
            return Err(format!("线程数需在 1~32，当前 {v}"));
        }
        if let Some(v) = self.chunk_size
            && !(400..=16_000).contains(&v)
        {
            return Err(format!("采样块大小需在 400~16000（@16k），当前 {v}"));
        }
        if let Some(v) = self.rule1_min_trailing_silence
            && !(0.0..=10.0).contains(&v)
        {
            return Err(format!("端点规则1尾随静音需在 0~10 秒，当前 {v}"));
        }
        if let Some(v) = self.rule2_min_trailing_silence
            && !(0.0..=10.0).contains(&v)
        {
            return Err(format!("端点规则2尾随静音需在 0~10 秒，当前 {v}"));
        }
        if let Some(v) = self.rule3_min_utterance_length
            && !(5.0..=60.0).contains(&v)
        {
            return Err(format!("端点最大句长需在 5~60 秒，当前 {v}"));
        }
        if let Some(v) = self.blank_penalty
            && !(0.0..=2.0).contains(&v)
        {
            return Err(format!("空白符惩罚需在 0~2，当前 {v}"));
        }

        if let Some(v) = self.num_threads {
            asr.num_threads = Some(v);
        }
        if let Some(v) = self.chunk_size {
            asr.chunk_size = Some(v);
        }
        if let Some(v) = self.enable_endpoint {
            asr.enable_endpoint = Some(v);
        }
        if let Some(v) = self.rule1_min_trailing_silence {
            asr.rule1_min_trailing_silence = Some(v);
        }
        if let Some(v) = self.rule2_min_trailing_silence {
            asr.rule2_min_trailing_silence = Some(v);
        }
        if let Some(v) = self.rule3_min_utterance_length {
            asr.rule3_min_utterance_length = Some(v);
        }
        if let Some(v) = self.blank_penalty {
            asr.blank_penalty = Some(v);
        }
        if let Some(v) = &self.hotwords {
            asr.hotwords = if v.trim().is_empty() {
                None
            } else {
                Some(v.trim().to_string())
            };
        }
        if let Some(v) = self.enable_punctuation {
            asr.enable_punctuation = Some(v);
        }
        if let Some(v) = self.debug {
            asr.debug = Some(v);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::AsrSettings;
    use crate::test_util::run_with_temp_home;

    #[test]
    fn test_default_model_dir_dual_root_fallback() {
        run_with_temp_home(|home| {
            crate::test_util::set_custom_data_dir(home);
            let new_dir = user_default_model_dir();
            let legacy_dir = home
                .join(".zapmomo")
                .join("models")
                .join(new_dir.file_name().unwrap());

            for d in [&new_dir, &legacy_dir] {
                std::fs::create_dir_all(d).unwrap();
                std::fs::write(d.join(DEFAULT_TOKENS), b"t").unwrap();
            }
            assert_eq!(default_model_dir(), new_dir);

            std::fs::remove_dir_all(&new_dir).unwrap();
            assert_eq!(default_model_dir(), legacy_dir);

            std::fs::remove_dir_all(&legacy_dir).unwrap();
            assert_ne!(default_model_dir(), legacy_dir);
        });
    }

    #[test]
    fn test_punctuation_default_dir_dual_root_fallback() {
        run_with_temp_home(|home| {
            crate::test_util::set_custom_data_dir(home);
            let new_punct = crate::kws::model::punctuation_user_model_dir();
            let legacy_punct = home
                .join(".zapmomo")
                .join("models")
                .join(new_punct.file_name().unwrap());

            // 只有旧根 → 默认标点模型指向旧根
            std::fs::create_dir_all(&legacy_punct).unwrap();
            std::fs::write(legacy_punct.join(DEFAULT_PUNCT_MODEL), b"x").unwrap();
            let cfg = resolve(None, None).unwrap();
            assert_eq!(
                cfg.punctuation_model,
                legacy_punct.join(DEFAULT_PUNCT_MODEL)
            );

            // 新根装好后切到新根
            std::fs::create_dir_all(&new_punct).unwrap();
            std::fs::write(new_punct.join(DEFAULT_PUNCT_MODEL), b"x").unwrap();
            let cfg2 = resolve(None, None).unwrap();
            assert_eq!(cfg2.punctuation_model, new_punct.join(DEFAULT_PUNCT_MODEL));
        });
    }

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

        assert_eq!(choose_default_model_dir(&user, None, &repo), user);

        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join(DEFAULT_TOKENS), b"t").unwrap();
        assert_eq!(choose_default_model_dir(&user, None, &repo), repo);

        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join(DEFAULT_TOKENS), b"t").unwrap();
        assert_eq!(choose_default_model_dir(&user, None, &repo), user);

        std::fs::remove_file(user.join(DEFAULT_TOKENS)).unwrap();
        let legacy = base.path().join("legacy-model");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join(DEFAULT_TOKENS), b"t").unwrap();
        assert_eq!(
            choose_default_model_dir(&user, Some(&legacy), &repo),
            legacy
        );
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
        // 用临时 HOME 隔离，避免与其它 `run_with_temp_home` 测试并行时 HOME 竞态
        // 导致 `resolve` 与 `ResolvedAsrConfig::default` 两次读取到不同 HOME。
        run_with_temp_home(|_| {
            let cfg = resolve(None, None).unwrap();
            assert_eq!(cfg, ResolvedAsrConfig::default());
        });
    }

    #[test]
    fn test_resolve_enabled_default_false_and_override() {
        run_with_temp_home(|_| {
            // 缺省 enabled=false
            assert!(!resolve(None, None).unwrap().enabled);
            // settings 显式启用
            let settings = AsrSettings {
                enabled: Some(true),
                ..Default::default()
            };
            assert!(resolve(Some(&settings), None).unwrap().enabled);
        });
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

    #[test]
    fn test_asr_params_patch_applies_valid_values() {
        let patch = AsrParamsPatch {
            num_threads: Some(8),
            chunk_size: Some(1600),
            enable_endpoint: Some(false),
            rule1_min_trailing_silence: Some(3.0),
            rule2_min_trailing_silence: Some(1.5),
            rule3_min_utterance_length: Some(30.0),
            blank_penalty: Some(0.5),
            hotwords: Some("你好小智 文森特卡索".to_string()),
            enable_punctuation: Some(false),
            debug: Some(true),
        };
        let mut asr = AsrSettings::default();
        patch.apply_to(&mut asr).unwrap();
        assert_eq!(asr.num_threads, Some(8));
        assert_eq!(asr.chunk_size, Some(1600));
        assert_eq!(asr.enable_endpoint, Some(false));
        assert_eq!(asr.rule1_min_trailing_silence, Some(3.0));
        assert_eq!(asr.rule2_min_trailing_silence, Some(1.5));
        assert_eq!(asr.rule3_min_utterance_length, Some(30.0));
        assert_eq!(asr.blank_penalty, Some(0.5));
        assert_eq!(asr.hotwords, Some("你好小智 文森特卡索".to_string()));
        assert_eq!(asr.enable_punctuation, Some(false));
        assert_eq!(asr.debug, Some(true));
    }

    #[test]
    fn test_asr_params_patch_rejects_out_of_range() {
        let cases: &[(&str, AsrParamsPatch)] = &[
            (
                "线程数",
                AsrParamsPatch {
                    num_threads: Some(0),
                    ..Default::default()
                },
            ),
            (
                "线程数",
                AsrParamsPatch {
                    num_threads: Some(33),
                    ..Default::default()
                },
            ),
            (
                "采样块大小",
                AsrParamsPatch {
                    chunk_size: Some(399),
                    ..Default::default()
                },
            ),
            (
                "采样块大小",
                AsrParamsPatch {
                    chunk_size: Some(16_001),
                    ..Default::default()
                },
            ),
            (
                "规则1",
                AsrParamsPatch {
                    rule1_min_trailing_silence: Some(-0.1),
                    ..Default::default()
                },
            ),
            (
                "规则2",
                AsrParamsPatch {
                    rule2_min_trailing_silence: Some(10.1),
                    ..Default::default()
                },
            ),
            (
                "最大句长",
                AsrParamsPatch {
                    rule3_min_utterance_length: Some(4.9),
                    ..Default::default()
                },
            ),
            (
                "空白符",
                AsrParamsPatch {
                    blank_penalty: Some(2.1),
                    ..Default::default()
                },
            ),
        ];
        for (label, patch) in cases {
            let mut asr = AsrSettings::default();
            let err = patch.apply_to(&mut asr).unwrap_err();
            assert!(
                err.contains(label),
                "参数「{label}」应被拒绝，实际错误: {err}"
            );
        }
    }

    #[test]
    fn test_asr_params_patch_all_or_nothing() {
        let mut asr = AsrSettings {
            num_threads: Some(4),
            ..AsrSettings::default()
        };
        let patch = AsrParamsPatch {
            num_threads: Some(16),
            chunk_size: Some(50_000), // 非法
            ..Default::default()
        };
        let err = patch.apply_to(&mut asr).unwrap_err();
        assert!(err.contains("采样块大小"));
        // 校验失败 → num_threads 未被写入（部分修改被阻止）
        assert_eq!(asr.num_threads, Some(4));
    }

    #[test]
    fn test_asr_params_patch_hotwords_empty_becomes_none() {
        let mut asr = AsrSettings::default();
        let patch = AsrParamsPatch {
            hotwords: Some("  ".to_string()),
            ..Default::default()
        };
        patch.apply_to(&mut asr).unwrap();
        assert_eq!(asr.hotwords, None);

        let mut asr2 = AsrSettings::default();
        let patch2 = AsrParamsPatch {
            hotwords: Some(" 你好  ".to_string()),
            ..Default::default()
        };
        patch2.apply_to(&mut asr2).unwrap();
        assert_eq!(asr2.hotwords, Some("你好".to_string()));
    }

    /// 双语布局（int8+fp32 混放，即全部注册包的实际布局）→ 常量直接命中，零行为变化。
    #[test]
    fn test_detect_asr_default_constants_win() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "encoder-epoch-99-avg-1.int8.onnx",
            "encoder-epoch-99-avg-1.onnx",
            "decoder-epoch-99-avg-1.int8.onnx",
            "decoder-epoch-99-avg-1.onnx",
            "joiner-epoch-99-avg-1.int8.onnx",
            "joiner-epoch-99-avg-1.onnx",
            DEFAULT_TOKENS,
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        assert_eq!(
            detect_default_onnx(dir.path(), "encoder", DEFAULT_ENCODER, true),
            DEFAULT_ENCODER
        );
        assert_eq!(
            detect_default_onnx(dir.path(), "decoder", DEFAULT_DECODER, false),
            DEFAULT_DECODER
        );
        assert_eq!(
            detect_default_onnx(dir.path(), "joiner", DEFAULT_JOINER, true),
            DEFAULT_JOINER
        );
        assert!(asr_files_present(dir.path()));
    }

    /// 非默认名混放（epoch-20 系，外部导入场景）→ encoder/joiner 取 int8、decoder 取 fp32。
    #[test]
    fn test_detect_asr_prefers_int8_encoder_joiner_fp32_decoder() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "encoder-epoch-20-avg-1-chunk-16-left-64.int8.onnx",
            "encoder-epoch-20-avg-1-chunk-16-left-64.onnx",
            "decoder-epoch-20-avg-1-chunk-16-left-64.int8.onnx",
            "decoder-epoch-20-avg-1-chunk-16-left-64.onnx",
            "joiner-epoch-20-avg-1-chunk-16-left-64.int8.onnx",
            "joiner-epoch-20-avg-1-chunk-16-left-64.onnx",
            DEFAULT_TOKENS,
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        assert_eq!(
            detect_default_onnx(dir.path(), "encoder", DEFAULT_ENCODER, true),
            "encoder-epoch-20-avg-1-chunk-16-left-64.int8.onnx"
        );
        assert_eq!(
            detect_default_onnx(dir.path(), "decoder", DEFAULT_DECODER, false),
            "decoder-epoch-20-avg-1-chunk-16-left-64.onnx"
        );
        assert_eq!(
            detect_default_onnx(dir.path(), "joiner", DEFAULT_JOINER, true),
            "joiner-epoch-20-avg-1-chunk-16-left-64.int8.onnx"
        );
        assert!(asr_files_present(dir.path()));
    }

    /// 单变体目录回退：int8-only 的 decoder 取 int8；fp32-only 的 encoder 取 fp32。
    #[test]
    fn test_detect_asr_fallback_to_any_variant() {
        let int8_only = tempfile::tempdir().unwrap();
        std::fs::write(
            int8_only.path().join("decoder-epoch-20-avg-1.int8.onnx"),
            b"x",
        )
        .unwrap();
        assert_eq!(
            detect_default_onnx(int8_only.path(), "decoder", DEFAULT_DECODER, false),
            "decoder-epoch-20-avg-1.int8.onnx"
        );

        let fp32_only = tempfile::tempdir().unwrap();
        std::fs::write(fp32_only.path().join("encoder-epoch-20-avg-1.onnx"), b"x").unwrap();
        assert_eq!(
            detect_default_onnx(fp32_only.path(), "encoder", DEFAULT_ENCODER, true),
            "encoder-epoch-20-avg-1.onnx"
        );
    }

    /// 目录不存在 / 空目录 / 无 onnx 候选 → 回退默认常量名。
    #[test]
    fn test_detect_asr_missing_or_empty_dir_falls_back_to_constant() {
        assert_eq!(
            detect_default_onnx(
                Path::new("/nonexistent-asr"),
                "encoder",
                DEFAULT_ENCODER,
                true
            ),
            DEFAULT_ENCODER
        );
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(
            detect_default_onnx(empty.path(), "encoder", DEFAULT_ENCODER, true),
            DEFAULT_ENCODER
        );
        // 前缀不匹配的 onnx 不算候选
        std::fs::write(empty.path().join("other-model.onnx"), b"x").unwrap();
        assert_eq!(
            detect_default_onnx(empty.path(), "encoder", DEFAULT_ENCODER, true),
            DEFAULT_ENCODER
        );
    }

    /// settings 只给 model_dir（切换模型的写法）+ 非默认命名目录 → resolve 命中探测名。
    #[test]
    fn test_resolve_detects_non_default_layout() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "encoder-epoch-20-avg-1-chunk-16-left-64.int8.onnx",
            "decoder-epoch-20-avg-1-chunk-16-left-64.onnx",
            "joiner-epoch-20-avg-1-chunk-16-left-64.int8.onnx",
            DEFAULT_TOKENS,
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let settings = AsrSettings {
            model_dir: Some(dir.path().to_string_lossy().to_string()),
            ..AsrSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(
            cfg.encoder,
            dir.path()
                .join("encoder-epoch-20-avg-1-chunk-16-left-64.int8.onnx")
        );
        assert_eq!(
            cfg.decoder,
            dir.path()
                .join("decoder-epoch-20-avg-1-chunk-16-left-64.onnx")
        );
        assert_eq!(
            cfg.joiner,
            dir.path()
                .join("joiner-epoch-20-avg-1-chunk-16-left-64.int8.onnx")
        );
        assert_eq!(cfg.tokens, dir.path().join(DEFAULT_TOKENS));
    }

    /// 显式 settings 文件覆盖优先，不探测（与 KWS resolve 语义一致）。
    #[test]
    fn test_resolve_explicit_file_overrides_skip_probe() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "encoder-epoch-20-avg-1-chunk-16-left-64.int8.onnx",
            DEFAULT_TOKENS,
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let settings = AsrSettings {
            model_dir: Some(dir.path().to_string_lossy().to_string()),
            encoder: Some("custom-encoder.onnx".to_string()),
            ..AsrSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(
            cfg.encoder,
            dir.path().join("custom-encoder.onnx"),
            "显式覆盖应直连，不被目录内文件影响"
        );
    }

    /// 探测式完整性判定：完整 true / 缺任一 false / 空目录 false / 不存在 false。
    #[test]
    fn test_asr_files_present() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!asr_files_present(dir.path()));
        assert!(!asr_files_present(Path::new("/nonexistent-asr")));

        for name in [
            "encoder-epoch-20-avg-1-chunk-16-left-64.int8.onnx",
            "decoder-epoch-20-avg-1-chunk-16-left-64.onnx",
            "joiner-epoch-20-avg-1-chunk-16-left-64.int8.onnx",
            DEFAULT_TOKENS,
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        assert!(asr_files_present(dir.path()));

        std::fs::remove_file(dir.path().join(DEFAULT_TOKENS)).unwrap();
        assert!(!asr_files_present(dir.path()), "缺 tokens 应为 false");

        std::fs::write(dir.path().join(DEFAULT_TOKENS), b"x").unwrap();
        std::fs::remove_file(
            dir.path()
                .join("encoder-epoch-20-avg-1-chunk-16-left-64.int8.onnx"),
        )
        .unwrap();
        assert!(!asr_files_present(dir.path()), "缺 encoder 应为 false");
    }
}
