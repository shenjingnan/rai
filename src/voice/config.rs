/// 语音会话配置解析。
///
/// 聚合 KWS / ASR / LLM / TTS 四个引擎配置（复用各自的 `config::resolve`）+
/// 会话级参数（唤醒词 / 音色 / 语速 / 轮数 / 打断等）。
/// 优先级与各引擎一致：CLI 覆盖 > settings.toml `[voice]` 段 > 内置默认。
use crate::config::settings::{AppConfig, VoiceSettings};
use std::path::PathBuf;

/// 默认历史消息条数上限（传给 LLM 的多轮上下文）。
pub const DEFAULT_HISTORY_MAX: usize = 12;
/// 默认打断 KWS 触发阈值（高于监听阈值 0.25，缓解回声误触发）。
pub const DEFAULT_BARGE_IN_THRESHOLD: f32 = 0.5;

/// CLI 覆盖参数（来自 `voice run` 命令行，缺省字段不覆盖 settings）。
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    /// 麦克风设备名（None = 系统默认或 settings.microphone）
    pub device: Option<String>,
    pub keywords: Option<String>,
    pub voice: Option<String>,
    pub speed: Option<f32>,
    pub max_turns: Option<u32>,
    pub history_max: Option<usize>,
    /// true = CLI 显式 `--no-bargein` 强制关闭打断（缺省 false 不强制，交给 settings）
    pub no_bargein: bool,
    pub barge_in_threshold: Option<f32>,
    pub kws_model_dir: Option<PathBuf>,
    pub asr_model_dir: Option<PathBuf>,
    pub tts_model_dir: Option<PathBuf>,
    pub llm_model_path: Option<PathBuf>,
}

/// 解析后的完整会话配置（字段全部为具体类型）。
#[derive(Debug, Clone)]
pub struct ResolvedSessionConfig {
    /// 麦克风设备名（None = 系统默认）
    pub mic_device: Option<String>,
    pub kws: crate::kws::config::ResolvedKwsConfig,
    pub asr: crate::asr::config::ResolvedAsrConfig,
    pub tts: crate::tts::config::ResolvedTtsConfig,
    pub llm: crate::llm::config::ResolvedLlmConfig,
    /// TTS 音色 id（None = 用 `tts` 配置默认参考音频）
    pub voice_id: Option<String>,
    /// TTS 语速
    pub speed: f32,
    /// 会话唤醒词（None = KWS 模型内置关键词）
    pub keywords: Option<String>,
    /// 最多对话轮数（None = 无限，Ctrl-C 退出）
    pub max_turns: Option<u32>,
    /// 传给 LLM 的历史消息条数上限
    pub history_max: usize,
    /// 播报/思考中唤醒词打断
    pub barge_in: bool,
    /// 打断用 KWS 触发阈值
    pub barge_in_threshold: f32,
}

/// 合并 settings 与 CLI 覆盖得到最终会话配置。
pub fn resolve(
    settings: Option<&AppConfig>,
    cli: &CliOverrides,
) -> Result<ResolvedSessionConfig, String> {
    let voice: Option<&VoiceSettings> = settings.and_then(|s| s.voice.as_ref());
    let kws = crate::kws::config::resolve(
        settings.and_then(|s| s.kws.as_ref()),
        cli.kws_model_dir.as_deref(),
    )?;
    let asr = crate::asr::config::resolve(
        settings.and_then(|s| s.asr.as_ref()),
        cli.asr_model_dir.as_deref(),
    )?;
    let tts = crate::tts::config::resolve(
        settings.and_then(|s| s.tts.as_ref()),
        cli.tts_model_dir.as_deref(),
    )?;
    let llm = crate::llm::config::resolve(
        settings.and_then(|s| s.llm.as_ref()),
        cli.llm_model_path.as_deref(),
    )?;

    Ok(ResolvedSessionConfig {
        // CLI --device > settings.microphone（全局）> 系统默认
        mic_device: cli
            .device
            .clone()
            .or_else(|| settings.and_then(|s| s.microphone.clone())),
        kws,
        asr,
        tts,
        llm,
        voice_id: cli
            .voice
            .clone()
            .or_else(|| voice.and_then(|v| v.voice.clone())),
        speed: cli
            .speed
            .or_else(|| voice.and_then(|v| v.speed))
            .unwrap_or(1.0),
        keywords: cli
            .keywords
            .clone()
            .or_else(|| voice.and_then(|v| v.keywords.clone())),
        max_turns: cli.max_turns.or_else(|| voice.and_then(|v| v.max_turns)),
        history_max: cli
            .history_max
            .or_else(|| voice.and_then(|v| v.history_max))
            .unwrap_or(DEFAULT_HISTORY_MAX),
        // CLI `--no-bargein` 强制关；未指定时尊重 settings（缺省开）
        barge_in: !cli.no_bargein && voice.and_then(|v| v.barge_in).unwrap_or(true),
        barge_in_threshold: cli
            .barge_in_threshold
            .or_else(|| voice.and_then(|v| v.barge_in_threshold))
            .unwrap_or(DEFAULT_BARGE_IN_THRESHOLD),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::run_with_temp_home;

    fn settings_with_voice(voice: VoiceSettings) -> AppConfig {
        AppConfig {
            voice: Some(voice),
            ..Default::default()
        }
    }

    #[test]
    fn test_defaults_with_no_settings() {
        run_with_temp_home(|_| {
            let cfg = resolve(None, &CliOverrides::default()).unwrap();
            assert_eq!(cfg.speed, 1.0);
            assert_eq!(cfg.history_max, DEFAULT_HISTORY_MAX);
            assert!(cfg.barge_in);
            assert_eq!(cfg.barge_in_threshold, DEFAULT_BARGE_IN_THRESHOLD);
            assert_eq!(cfg.voice_id, None);
            assert_eq!(cfg.keywords, None);
            assert_eq!(cfg.max_turns, None);
        });
    }

    #[test]
    fn test_settings_voice_section_overrides() {
        run_with_temp_home(|_| {
            let voice = VoiceSettings {
                keywords: Some("你好小智".to_string()),
                voice: Some("news-female".to_string()),
                speed: Some(1.2),
                max_turns: Some(5),
                history_max: Some(20),
                barge_in: Some(false),
                barge_in_threshold: Some(0.7),
            };
            let cfg = resolve(Some(&settings_with_voice(voice)), &CliOverrides::default()).unwrap();
            assert_eq!(cfg.keywords.as_deref(), Some("你好小智"));
            assert_eq!(cfg.voice_id.as_deref(), Some("news-female"));
            assert_eq!(cfg.speed, 1.2);
            assert_eq!(cfg.max_turns, Some(5));
            assert_eq!(cfg.history_max, 20);
            assert!(!cfg.barge_in);
            assert_eq!(cfg.barge_in_threshold, 0.7);
        });
    }

    #[test]
    fn test_cli_overrides_settings() {
        run_with_temp_home(|_| {
            let voice = VoiceSettings {
                keywords: Some("settings词".to_string()),
                voice: Some("leijun-1".to_string()),
                speed: Some(0.8),
                ..Default::default()
            };
            let cli = CliOverrides {
                keywords: Some("cli词".to_string()),
                voice: Some("news-female-2".to_string()),
                speed: Some(1.5),
                ..Default::default()
            };
            let cfg = resolve(Some(&settings_with_voice(voice)), &cli).unwrap();
            assert_eq!(cfg.keywords.as_deref(), Some("cli词"));
            assert_eq!(cfg.voice_id.as_deref(), Some("news-female-2"));
            assert_eq!(cfg.speed, 1.5);
        });
    }

    #[test]
    fn test_cli_no_bargein_forces_off() {
        run_with_temp_home(|_| {
            // settings 缺省开，CLI --no-bargein 强制关
            let cli = CliOverrides {
                no_bargein: true,
                ..Default::default()
            };
            let cfg = resolve(None, &cli).unwrap();
            assert!(!cfg.barge_in);
        });
    }

    #[test]
    fn test_barge_in_respects_settings_when_cli_unset() {
        run_with_temp_home(|_| {
            // CLI 缺省（barge_in=true）时，settings barge_in=false 生效
            let voice = VoiceSettings {
                barge_in: Some(false),
                ..Default::default()
            };
            let cfg = resolve(Some(&settings_with_voice(voice)), &CliOverrides::default()).unwrap();
            assert!(!cfg.barge_in);
        });
    }

    #[test]
    fn test_voice_settings_serde_roundtrip() {
        run_with_temp_home(|_| {
            let voice = VoiceSettings {
                keywords: Some("你好小智".to_string()),
                voice: Some("leijun-1".to_string()),
                speed: Some(1.1),
                max_turns: Some(10),
                history_max: Some(16),
                barge_in: Some(true),
                barge_in_threshold: Some(0.6),
            };
            let app = settings_with_voice(voice);
            let toml_str = toml::to_string(&app).unwrap();
            let loaded: AppConfig = toml::from_str(&toml_str).unwrap();
            assert_eq!(loaded.voice, app.voice);
        });
    }
}
