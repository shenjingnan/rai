use crate::config::settings::{TtsSettings, resolve_env_ref};
/// TTS 配置解析与校验。
///
/// 负责把 `settings.toml` 的 `[tts]` 表与 CLI flag 合并成一份已展开、已填默认值的
/// `ResolvedTtsConfig`。优先级：CLI `--model-dir` > settings > 内置默认。
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 模型包内默认文件名（sherpa-onnx 官方 zipvoice distill int8 打包版）。
pub const DEFAULT_ENCODER: &str = "encoder.int8.onnx";
pub const DEFAULT_DECODER: &str = "decoder.int8.onnx";
/// 声码器（独立发布，`tts install-model` 时与主包一并下载）。
pub const DEFAULT_VOCODER: &str = "vocos_24khz.onnx";
pub const DEFAULT_TOKENS: &str = "tokens.txt";
pub const DEFAULT_LEXICON: &str = "lexicon.txt";
/// espeak-ng 数据目录（相对模型目录）。
pub const DEFAULT_DATA_DIR: &str = "espeak-ng-data";
/// 默认参考音频（零样本声音克隆的音色来源）。
pub const DEFAULT_REFERENCE_WAV: &str = "test_wavs/leijun-1.wav";
/// 默认参考音频的逐字转写（来自模型包内 test_wavs/prompt.txt）。
pub const DEFAULT_REFERENCE_TEXT: &str = "那还是36年前, 1987年. 我呢考上了武汉大学的计算机系.";

/// ZipVoice 官方示例推荐参数（Rust 的 `Default` 全为 0，需显式设置）。
pub const DEFAULT_FEAT_SCALE: f32 = 0.1;
pub const DEFAULT_T_SHIFT: f32 = 0.5;
pub const DEFAULT_TARGET_RMS: f32 = 0.1;
pub const DEFAULT_GUIDANCE_SCALE: f32 = 1.0;

/// 模型安装完成所需的文件（相对目标目录；espeak-ng-data 目录与参考 wav 由引擎单独校验）。
pub const REQUIRED_FILES: [&str; 5] = [
    DEFAULT_ENCODER,
    DEFAULT_DECODER,
    DEFAULT_VOCODER,
    DEFAULT_TOKENS,
    DEFAULT_LEXICON,
];

/// 解析后的完整 TTS 配置。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTtsConfig {
    /// 是否启用语音合成
    pub enabled: bool,
    pub model_dir: PathBuf,
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub vocoder: PathBuf,
    pub tokens: PathBuf,
    pub lexicon: PathBuf,
    pub data_dir: PathBuf,
    pub reference_wav: PathBuf,
    pub reference_text: String,
    /// 默认音色 id（如 `leijun-1` / 自定义音色 id；None = 用 reference_wav 即 leijun）。
    pub voice: Option<String>,
    /// 扩散解码步数（质量/速度权衡）
    pub num_steps: i32,
    /// 语速
    pub speed: f32,
    pub provider: String,
    pub num_threads: i32,
    pub debug: bool,
}

impl Default for ResolvedTtsConfig {
    fn default() -> Self {
        let model_dir = default_model_dir();
        let join = |name: &str| model_dir.join(name);
        Self {
            enabled: true,
            encoder: join(DEFAULT_ENCODER),
            decoder: join(DEFAULT_DECODER),
            vocoder: join(DEFAULT_VOCODER),
            tokens: join(DEFAULT_TOKENS),
            lexicon: join(DEFAULT_LEXICON),
            data_dir: join(DEFAULT_DATA_DIR),
            reference_wav: join(DEFAULT_REFERENCE_WAV),
            model_dir,
            reference_text: DEFAULT_REFERENCE_TEXT.to_string(),
            voice: None,
            num_steps: 4,
            speed: 1.0,
            provider: "cpu".to_string(),
            num_threads: 2,
            debug: false,
        }
    }
}

/// 用户默认模型目录：`~/.zapmomo/models/<模型名>`
pub fn user_default_model_dir() -> PathBuf {
    crate::kws::model::tts_user_model_dir()
}

/// 源码仓库中的模型目录（开发者 `./models/<模型名>`，仅作开发回退）。
fn repo_models_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join(&crate::kws::model::tts_asset().name)
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
        .map(|l| l.join(&crate::kws::model::tts_asset().name));
    choose_default_model_dir(
        &user_default_model_dir(),
        legacy.as_deref(),
        &repo_models_dir(),
    )
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
    settings: Option<&TtsSettings>,
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
    settings: Option<&TtsSettings>,
    cli_model_dir: Option<&Path>,
) -> Result<ResolvedTtsConfig, String> {
    let mut cfg = ResolvedTtsConfig {
        model_dir: resolve_model_dir(settings, cli_model_dir)?,
        ..ResolvedTtsConfig::default()
    };

    let s = settings;
    cfg.enabled = s.and_then(|s| s.enabled).unwrap_or(true);
    let file = |field: &str, default_name: &str| {
        let value = match field {
            "encoder" => s.and_then(|s| s.encoder.as_deref()),
            "decoder" => s.and_then(|s| s.decoder.as_deref()),
            "vocoder" => s.and_then(|s| s.vocoder.as_deref()),
            "tokens" => s.and_then(|s| s.tokens.as_deref()),
            "lexicon" => s.and_then(|s| s.lexicon.as_deref()),
            "data_dir" => s.and_then(|s| s.data_dir.as_deref()),
            "reference_wav" => s.and_then(|s| s.reference_wav.as_deref()),
            _ => None,
        };
        resolve_file(value, default_name, &cfg.model_dir)
    };

    cfg.encoder = file("encoder", DEFAULT_ENCODER)?;
    cfg.decoder = file("decoder", DEFAULT_DECODER)?;
    cfg.vocoder = file("vocoder", DEFAULT_VOCODER)?;
    cfg.tokens = file("tokens", DEFAULT_TOKENS)?;
    cfg.lexicon = file("lexicon", DEFAULT_LEXICON)?;
    cfg.data_dir = file("data_dir", DEFAULT_DATA_DIR)?;
    cfg.reference_wav = file("reference_wav", DEFAULT_REFERENCE_WAV)?;

    cfg.reference_text = s
        .and_then(|s| s.reference_text.clone())
        .unwrap_or_else(|| DEFAULT_REFERENCE_TEXT.to_string());
    cfg.voice = s.and_then(|s| s.voice.clone());
    cfg.num_steps = s.and_then(|s| s.num_steps).unwrap_or(4);
    cfg.speed = s.and_then(|s| s.speed).unwrap_or(1.0);
    cfg.provider = s
        .and_then(|s| s.provider.clone())
        .unwrap_or_else(|| "cpu".to_string());
    cfg.num_threads = s.and_then(|s| s.num_threads).unwrap_or(2);
    cfg.debug = s.and_then(|s| s.debug).unwrap_or(false);

    Ok(cfg)
}

/// `set_tts_params` 载荷：可调整的 TTS 合成参数（snake_case 直传，缺省项不修改）。
///
/// 与 `AsrParamsPatch` 对称，放在 lib crate 内以便 `cargo test` 单测。
/// 引擎在每次合成时新建（`synthesize_tts` → `TtsEngine::new`），因此保存后**下一次合成即生效**，无需重启。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TtsParamsPatch {
    /// 扩散解码步数（质量/速度权衡）
    pub num_steps: Option<i32>,
    /// 默认语速（单次合成可经 `synthesize_tts.speed` 覆盖）
    pub speed: Option<f32>,
    /// 推理线程数
    pub num_threads: Option<i32>,
    /// 调试输出
    pub debug: Option<bool>,
}

impl TtsParamsPatch {
    /// 先整体校验（任一越界立即 Err），再逐项写入 `TtsSettings`，保证出错时不部分修改。
    pub fn apply_to(&self, tts: &mut TtsSettings) -> Result<(), String> {
        if let Some(v) = self.num_steps
            && !(1..=32).contains(&v)
        {
            return Err(format!("扩散步数需在 1~32，当前 {v}"));
        }
        if let Some(v) = self.speed
            && !(0.5..=2.0).contains(&v)
        {
            return Err(format!("语速需在 0.5~2.0，当前 {v}"));
        }
        if let Some(v) = self.num_threads
            && !(1..=32).contains(&v)
        {
            return Err(format!("线程数需在 1~32，当前 {v}"));
        }

        if let Some(v) = self.num_steps {
            tts.num_steps = Some(v);
        }
        if let Some(v) = self.speed {
            tts.speed = Some(v);
        }
        if let Some(v) = self.num_threads {
            tts.num_threads = Some(v);
        }
        if let Some(v) = self.debug {
            tts.debug = Some(v);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::TtsSettings;
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
    fn test_default_config_points_to_default_model_dir() {
        let cfg = ResolvedTtsConfig::default();
        assert_eq!(
            cfg.model_dir
                .file_name()
                .map(|s| s.to_string_lossy().to_string()),
            Some(crate::kws::model::tts_asset().name.clone())
        );
        assert_eq!(cfg.encoder.file_name().unwrap(), DEFAULT_ENCODER);
        assert_eq!(cfg.decoder.file_name().unwrap(), DEFAULT_DECODER);
        assert_eq!(cfg.vocoder.file_name().unwrap(), DEFAULT_VOCODER);
        assert_eq!(cfg.tokens.file_name().unwrap(), DEFAULT_TOKENS);
        assert_eq!(cfg.lexicon.file_name().unwrap(), DEFAULT_LEXICON);
        assert_eq!(cfg.data_dir.file_name().unwrap(), DEFAULT_DATA_DIR);
        assert_eq!(cfg.reference_wav.file_name().unwrap(), "leijun-1.wav");
        assert_eq!(cfg.reference_text, DEFAULT_REFERENCE_TEXT);
        assert_eq!(cfg.num_steps, 4);
        assert_eq!(cfg.speed, 1.0);
        assert_eq!(cfg.provider, "cpu");
    }

    #[test]
    fn test_user_default_model_dir() {
        run_with_temp_home(|home| {
            let dir = super::user_default_model_dir();
            assert_eq!(
                dir,
                home.join(".zapmomo/models")
                    .join(crate::kws::model::tts_asset().name.as_str())
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
    fn test_resolve_enabled_default_true_and_override() {
        // 未配置时默认启用，避免破坏现有用户
        assert!(resolve(None, None).unwrap().enabled);
        // settings 显式关闭时生效
        let settings = TtsSettings {
            enabled: Some(false),
            ..TtsSettings::default()
        };
        assert!(!resolve(Some(&settings), None).unwrap().enabled);
    }

    #[test]
    fn test_resolve_no_settings_uses_defaults() {
        // 用临时 HOME 隔离，避免与其它 `run_with_temp_home` 测试并行时 HOME 竞态
        // 导致 `resolve` 与 `ResolvedTtsConfig::default` 两次读取到不同 HOME。
        run_with_temp_home(|_| {
            let cfg = resolve(None, None).unwrap();
            assert_eq!(cfg, ResolvedTtsConfig::default());
        });
    }

    fn abs_path(rel: &str) -> PathBuf {
        std::path::absolute(rel).unwrap()
    }

    #[test]
    fn test_resolve_cli_model_dir_overrides_settings() {
        let settings = TtsSettings {
            model_dir: Some("settings-model".to_string()),
            ..TtsSettings::default()
        };
        let cli = abs_path("tmp/cli-tts");
        let cfg = resolve(Some(&settings), Some(&cli)).unwrap();
        assert_eq!(cfg.model_dir, cli);
        assert_eq!(cfg.encoder.parent().unwrap(), cli);
    }

    #[test]
    fn test_resolve_settings_overrides_numeric_and_text() {
        let settings = TtsSettings {
            num_threads: Some(4),
            num_steps: Some(6),
            speed: Some(1.5),
            reference_text: Some("自定义参考文本".to_string()),
            debug: Some(true),
            ..TtsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.num_threads, 4);
        assert_eq!(cfg.num_steps, 6);
        assert_eq!(cfg.speed, 1.5);
        assert_eq!(cfg.reference_text, "自定义参考文本");
        assert!(cfg.debug);
    }

    #[test]
    fn test_resolve_relative_model_dir_anchored_to_user_dir() {
        run_with_temp_home(|home| {
            let settings = TtsSettings {
                model_dir: Some("models/my-tts".to_string()),
                ..TtsSettings::default()
            };
            let cfg = resolve(Some(&settings), None).unwrap();
            assert_eq!(cfg.model_dir, home.join(".zapmomo/models/my-tts"));
        });
    }

    #[test]
    fn test_resolve_voice_default_none_and_override() {
        // 未配置默认音色 → None（用 reference_wav 即 leijun）
        let cfg = resolve(None, None).unwrap();
        assert_eq!(cfg.voice, None);
        // settings 配置音色 id → 解析生效
        let settings = TtsSettings {
            voice: Some("custom-123".to_string()),
            ..TtsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.voice.as_deref(), Some("custom-123"));
    }

    #[test]
    fn test_required_files_count_and_content() {
        assert_eq!(REQUIRED_FILES.len(), 5);
        assert!(REQUIRED_FILES.contains(&DEFAULT_VOCODER));
        assert!(REQUIRED_FILES.contains(&DEFAULT_LEXICON));
    }

    #[test]
    fn test_params_patch_applies_all_fields() {
        let mut tts = TtsSettings::default();
        let patch = TtsParamsPatch {
            num_steps: Some(8),
            speed: Some(1.2),
            num_threads: Some(4),
            debug: Some(true),
        };
        patch.apply_to(&mut tts).unwrap();
        assert_eq!(tts.num_steps, Some(8));
        assert_eq!(tts.speed, Some(1.2));
        assert_eq!(tts.num_threads, Some(4));
        assert_eq!(tts.debug, Some(true));
    }

    #[test]
    fn test_params_patch_validates_before_writing() {
        // 任一字段越界即整体失败，且不部分修改其它字段
        let mut tts = TtsSettings {
            num_steps: Some(4),
            ..TtsSettings::default()
        };
        let err = TtsParamsPatch {
            num_steps: Some(100),
            num_threads: Some(4),
            ..TtsParamsPatch::default()
        }
        .apply_to(&mut tts)
        .unwrap_err();
        assert!(err.contains("扩散步数"), "err: {err}");
        assert_eq!(tts.num_threads, None, "校验失败时不应写入其它字段");
        assert_eq!(tts.num_steps, Some(4));

        let err = TtsParamsPatch {
            speed: Some(3.0),
            ..TtsParamsPatch::default()
        }
        .apply_to(&mut TtsSettings::default())
        .unwrap_err();
        assert!(err.contains("语速"), "err: {err}");

        let err = TtsParamsPatch {
            num_threads: Some(64),
            ..TtsParamsPatch::default()
        }
        .apply_to(&mut TtsSettings::default())
        .unwrap_err();
        assert!(err.contains("线程数"), "err: {err}");
    }

    #[test]
    fn test_params_patch_none_leaves_unchanged() {
        let mut tts = TtsSettings {
            num_steps: Some(6),
            speed: Some(1.5),
            num_threads: Some(8),
            debug: Some(true),
            ..TtsSettings::default()
        };
        TtsParamsPatch::default().apply_to(&mut tts).unwrap();
        assert_eq!(tts.num_steps, Some(6));
        assert_eq!(tts.speed, Some(1.5));
        assert_eq!(tts.num_threads, Some(8));
        assert_eq!(tts.debug, Some(true));
    }
}
