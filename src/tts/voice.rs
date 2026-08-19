/// TTS 音色（参考音色）列表。
///
/// ZipVoice 是零样本声音克隆模型，音色 = 参考音频 + 参考文本。内置音色来自
/// 模型包内 `test_wavs/prompt.txt`（每行 `<wav文件名> <转写文本>`），运行时解析。
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::tts::config::ResolvedTtsConfig;

/// 一个可用音色。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TtsVoice {
    /// 唯一标识（wav 文件名去 `.wav` 后缀，如 `leijun-1`）。
    pub id: String,
    /// 显示名（内置音色有友好中文名，否则用 id）。
    pub name: String,
    /// 参考音频绝对路径。
    pub wav_path: PathBuf,
    /// 参考音频的逐字转写文本。
    pub reference_text: String,
    /// 是否为用户自定义音色（true = 来自音色库，false = 模型包内置）。
    pub custom: bool,
}

/// 内置音色的友好中文名（prompt.txt 只有文件名，这里做一层展示映射）。
fn friendly_name(id: &str) -> String {
    match id {
        "leijun-1" => "雷军（男）".to_string(),
        "news-female" => "新闻女声".to_string(),
        "news-female-2" => "新闻女声 2".to_string(),
        _ => id.to_string(),
    }
}

/// 解析 `test_wavs/prompt.txt` 的一行。
fn parse_prompt_line(line: &str, model_dir: &Path) -> Option<TtsVoice> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (wav_name, text) = line.split_once(' ')?;
    let wav_name = wav_name.trim();
    let text = text.trim();
    if wav_name.is_empty() || text.is_empty() || !wav_name.ends_with(".wav") {
        return None;
    }
    let id = wav_name.trim_end_matches(".wav").to_string();
    Some(TtsVoice {
        name: friendly_name(&id),
        id,
        wav_path: model_dir.join("test_wavs").join(wav_name),
        reference_text: text.to_string(),
        custom: false,
    })
}

/// 列出模型包内置的参考音色（解析 `<model_dir>/test_wavs/prompt.txt`）。
pub fn list_builtin_voices(model_dir: &Path) -> Vec<TtsVoice> {
    let prompt = model_dir.join("test_wavs").join("prompt.txt");
    let Ok(content) = std::fs::read_to_string(&prompt) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| parse_prompt_line(line, model_dir))
        .collect()
}

/// 按 id 从音色列表中查找。
pub fn find_voice<'a>(voices: &'a [TtsVoice], id: &str) -> Option<&'a TtsVoice> {
    voices.iter().find(|v| v.id == id)
}

/// 解析最终参考音色：自定义 wav > 自定义音色（id/名称）> 内置音色 id > 配置默认。
pub fn resolve_reference(
    cfg: &ResolvedTtsConfig,
    voice_id: Option<&str>,
    custom_wav: Option<&Path>,
    custom_text: Option<&str>,
) -> Result<(PathBuf, String), String> {
    if let Some(wav) = custom_wav {
        let text = custom_text
            .ok_or_else(|| "自定义参考音频必须同时提供参考文本（逐字转写）".to_string())?;
        return Ok((wav.to_path_buf(), text.to_string()));
    }
    if let Some(id) = voice_id {
        // 优先匹配用户自定义音色（音色库，支持按 id 或展示名）
        if let Some(v) = crate::tts::voice_store::list_custom_voices()
            .into_iter()
            .find(|v| v.id == id || v.name == id)
        {
            return Ok((v.wav_path, v.reference_text));
        }
        // 再匹配模型包内置音色
        let voices = list_builtin_voices(&cfg.model_dir);
        let v = find_voice(&voices, id).ok_or_else(|| format!("未找到音色: {id}"))?;
        return Ok((v.wav_path.clone(), v.reference_text.clone()));
    }
    Ok((cfg.reference_wav.clone(), cfg.reference_text.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_prompt(model_dir: &Path, content: &str) {
        std::fs::create_dir_all(model_dir.join("test_wavs")).unwrap();
        std::fs::write(model_dir.join("test_wavs/prompt.txt"), content).unwrap();
    }

    #[test]
    fn test_list_builtin_voices_parses_prompt() {
        let dir = tempfile::tempdir().unwrap();
        make_prompt(
            dir.path(),
            "leijun-1.wav 那还是36年前, 1987年. 我呢考上了武汉大学的计算机系.\n\
             news-female.wav 各位村民, 大家新年好! 近期, 湖北省武汉市等多个地区\n\
             news-female-2.wav 本台消息, 中共中央国务院, 近日印发关于构建数据基础制度.\n",
        );
        let voices = list_builtin_voices(dir.path());
        assert_eq!(voices.len(), 3);

        let leijun = find_voice(&voices, "leijun-1").unwrap();
        assert_eq!(leijun.name, "雷军（男）");
        assert_eq!(leijun.wav_path, dir.path().join("test_wavs/leijun-1.wav"));
        assert!(leijun.reference_text.contains("计算机系"));

        let news = find_voice(&voices, "news-female").unwrap();
        assert_eq!(news.name, "新闻女声");
    }

    #[test]
    fn test_list_builtin_voices_skips_invalid_lines() {
        let dir = tempfile::tempdir().unwrap();
        make_prompt(
            dir.path(),
            "\n\nmissing-text.wav\nno-extension 文本\nleijun-1.wav 有效的参考文本\n",
        );
        let voices = list_builtin_voices(dir.path());
        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].id, "leijun-1");
    }

    #[test]
    fn test_list_builtin_voices_missing_prompt_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let voices = list_builtin_voices(dir.path());
        assert!(voices.is_empty());
    }

    /// 生成一个合法最小 wav（RIFF 头 + 少量样本），满足 `voice_store::save_voice` 校验。
    fn sample_wav_bytes() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&44u32.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&16000u32.to_le_bytes());
        buf.extend_from_slice(&32000u32.to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes());
        buf.extend_from_slice(&16u16.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&8u32.to_le_bytes());
        buf.extend_from_slice(&0i16.to_le_bytes());
        buf.extend_from_slice(&0i16.to_le_bytes());
        buf
    }

    #[test]
    fn test_resolve_reference_custom_voice_by_name() {
        crate::test_util::run_with_temp_home(|home| {
            let src = home.join("src.wav");
            std::fs::write(&src, sample_wav_bytes()).unwrap();
            let v = crate::tts::voice_store::save_voice("大月下", &src, "为什么人类要起这么早啊")
                .unwrap();

            let cfg = ResolvedTtsConfig::default();
            let (wav, text) = resolve_reference(&cfg, Some("大月下"), None, None).unwrap();
            assert_eq!(wav, v.wav_path);
            assert_eq!(text, "为什么人类要起这么早啊");
        });
    }

    #[test]
    fn test_resolve_reference_custom_voice_by_id() {
        crate::test_util::run_with_temp_home(|home| {
            let src = home.join("src.wav");
            std::fs::write(&src, sample_wav_bytes()).unwrap();
            let v = crate::tts::voice_store::save_voice("大月下", &src, "参考文本").unwrap();

            let cfg = ResolvedTtsConfig::default();
            let (wav, text) = resolve_reference(&cfg, Some(&v.id), None, None).unwrap();
            assert_eq!(wav, v.wav_path);
            assert_eq!(text, "参考文本");
        });
    }

    #[test]
    fn test_resolve_reference_builtin_still_works() {
        let dir = tempfile::tempdir().unwrap();
        make_prompt(dir.path(), "leijun-1.wav 那还是36年前, 1987年.\n");
        let cfg = ResolvedTtsConfig {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let (wav, text) = resolve_reference(&cfg, Some("leijun-1"), None, None).unwrap();
        assert_eq!(wav, dir.path().join("test_wavs/leijun-1.wav"));
        assert!(text.contains("1987年"));
    }

    #[test]
    fn test_resolve_reference_unknown_voice_errors() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = ResolvedTtsConfig {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let err = resolve_reference(&cfg, Some("不存在的音色"), None, None).unwrap_err();
        assert!(err.contains("未找到音色"), "err: {err}");
    }

    #[test]
    fn test_resolve_reference_custom_wav_requires_text() {
        let cfg = ResolvedTtsConfig::default();
        let err = resolve_reference(&cfg, None, Some(Path::new("/tmp/a.wav")), None).unwrap_err();
        assert!(err.contains("参考文本"), "err: {err}");
    }
}
