/// 文本转语音（TTS）。
///
/// 使用 sherpa-onnx 的 `OfflineTts`（ZipVoice 零样本声音克隆，中英双语）实现：
/// 输入文本 → 合成 PCM 波形（可落盘 wav）。批量一次性合成，无流式 feed 循环。
///
/// 设计上对齐 KWS/ASR：模型清单下载、配置解析、引擎「逐文件预检 + install-model 提示」
/// 等模式保持一致；进度通过 `generate_with_config` 的回调（`FnMut(f32) -> bool`）暴露。
pub mod config;
pub mod reaction;
pub mod voice;
pub mod voice_store;

use config::ResolvedTtsConfig;
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsZipvoiceModelConfig, Wave,
};
use std::path::{Path, PathBuf};

pub use crate::kws::model::{DownloadProgress, DownloadStage, ModelError, ProgressFn};
pub use voice::TtsVoice;

/// 文本转语音引擎。
///
/// 持有 `OfflineTts`。参考音色（零样本声音克隆的音色来源）在每次合成时按需传入，
/// 因此引擎可复用、可切换音色。所有方法接收 `&self`。
pub struct TtsEngine {
    tts: OfflineTts,
    cfg: ResolvedTtsConfig,
}

impl TtsEngine {
    /// 构造引擎，先校验所有模型文件存在。
    pub fn new(cfg: ResolvedTtsConfig) -> Result<Self, String> {
        let required_files = [
            ("encoder", &cfg.encoder),
            ("decoder", &cfg.decoder),
            ("vocoder", &cfg.vocoder),
            ("tokens", &cfg.tokens),
            ("lexicon", &cfg.lexicon),
        ];
        for (name, path) in required_files {
            if !path.is_file() {
                return Err(format!(
                    "缺少模型文件 {name}: {}\n请运行 `zapmomo tts install-model` 下载模型。",
                    path.display()
                ));
            }
        }
        if !cfg.data_dir.is_dir() {
            return Err(format!(
                "缺少数据目录 data_dir: {}\n请运行 `zapmomo tts install-model` 下载模型。",
                cfg.data_dir.display()
            ));
        }

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                zipvoice: OfflineTtsZipvoiceModelConfig {
                    tokens: Some(cfg.tokens.to_string_lossy().to_string()),
                    encoder: Some(cfg.encoder.to_string_lossy().to_string()),
                    decoder: Some(cfg.decoder.to_string_lossy().to_string()),
                    vocoder: Some(cfg.vocoder.to_string_lossy().to_string()),
                    data_dir: Some(cfg.data_dir.to_string_lossy().to_string()),
                    lexicon: Some(cfg.lexicon.to_string_lossy().to_string()),
                    feat_scale: config::DEFAULT_FEAT_SCALE,
                    t_shift: config::DEFAULT_T_SHIFT,
                    target_rms: config::DEFAULT_TARGET_RMS,
                    guidance_scale: config::DEFAULT_GUIDANCE_SCALE,
                },
                num_threads: cfg.num_threads,
                debug: cfg.debug,
                provider: Some(cfg.provider.clone()),
                ..Default::default()
            },
            ..Default::default()
        };

        let tts = OfflineTts::create(&config)
            .ok_or_else(|| "无法创建 OfflineTts，请检查模型文件与配置。".to_string())?;

        Ok(Self { tts, cfg })
    }

    pub fn config(&self) -> &ResolvedTtsConfig {
        &self.cfg
    }

    /// 合成输出的采样率（Hz）。
    pub fn sample_rate(&self) -> i32 {
        self.tts.sample_rate()
    }

    /// 加载参考音频并构建生成配置（参考音色 + 参考文本 + 扩散步数 + 语速）。
    fn generation_config(
        &self,
        speed: f32,
        reference_wav: &Path,
        reference_text: &str,
    ) -> Result<GenerationConfig, String> {
        let wave = Wave::read(&reference_wav.to_string_lossy())
            .ok_or_else(|| format!("无法读取参考音频: {}", reference_wav.display()))?;
        // 把参考音频归一化到模型目标采样率：ZipVoice 的 Mel 频谱在跨采样率
        // （如 48k→24k）时，sherpa C++ 重采样器可能抛异常，Rust 无法捕获
        // C++ 异常会直接 abort。统一到目标采样率后 Mel 重采样变为恒等变换。
        let (reference_audio, reference_sample_rate) =
            normalize_reference(wave.samples(), wave.sample_rate(), self.sample_rate())?;
        Ok(GenerationConfig {
            speed,
            reference_audio: Some(reference_audio),
            reference_sample_rate,
            reference_text: Some(reference_text.to_string()),
            num_steps: self.cfg.num_steps,
            ..Default::default()
        })
    }

    /// 把文本合成为 PCM 波形（f32，采样率见 [`Self::sample_rate`]）。
    ///
    /// 模型始终以 1.0 语速合成（避免 ZipVoice 高语速时 `kept_frames≤0` 触发
    /// sherpa C++ 异常导致 Rust abort），目标语速通过对输出重采样实现。
    pub fn synthesize(
        &self,
        text: &str,
        speed: f32,
        reference_wav: &Path,
        reference_text: &str,
    ) -> Result<Vec<f32>, String> {
        let gen_config = self.generation_config(1.0, reference_wav, reference_text)?;
        let audio = self
            .tts
            .generate_with_config(text, &gen_config, None::<fn(&[f32], f32) -> bool>)
            .ok_or_else(|| "语音合成失败。".to_string())?;
        apply_speed_to_samples(audio.samples(), self.sample_rate(), speed)
    }

    /// 把文本合成为 PCM，并在合成过程中回调进度（0..1）。
    ///
    /// `progress` 返回 `false` 提前终止合成（对应 sherpa-onnx 回调语义）。
    /// 语速同 [`Self::synthesize`]：模型按 1.0 合成，输出重采样实现。
    pub fn synthesize_with_progress<F>(
        &self,
        text: &str,
        speed: f32,
        reference_wav: &Path,
        reference_text: &str,
        mut progress: F,
    ) -> Result<Vec<f32>, String>
    where
        F: FnMut(f32) -> bool + 'static,
    {
        let gen_config = self.generation_config(1.0, reference_wav, reference_text)?;
        let callback = move |_samples: &[f32], p: f32| progress(p);
        let audio = self
            .tts
            .generate_with_config(text, &gen_config, Some(callback))
            .ok_or_else(|| "语音合成失败。".to_string())?;
        apply_speed_to_samples(audio.samples(), self.sample_rate(), speed)
    }

    /// 把文本合成为 wav 文件。
    pub fn synthesize_to_wav(
        &self,
        text: &str,
        speed: f32,
        reference_wav: &Path,
        reference_text: &str,
        out_path: &Path,
    ) -> Result<(), String> {
        self.synthesize_to_wav_with_progress(
            text,
            speed,
            reference_wav,
            reference_text,
            out_path,
            |_p| true,
        )
        .map(|_| ())
    }

    /// 把文本合成为 wav 文件，并在合成过程中回调进度（0..1）。
    ///
    /// 返回采样点数（已应用语速），便于调用方换算音频时长（`samples / sample_rate`）。
    pub fn synthesize_to_wav_with_progress<F>(
        &self,
        text: &str,
        speed: f32,
        reference_wav: &Path,
        reference_text: &str,
        out_path: &Path,
        mut progress: F,
    ) -> Result<usize, String>
    where
        F: FnMut(f32) -> bool + 'static,
    {
        let gen_config = self.generation_config(1.0, reference_wav, reference_text)?;
        let callback = move |_samples: &[f32], p: f32| progress(p);
        let audio = self
            .tts
            .generate_with_config(text, &gen_config, Some(callback))
            .ok_or_else(|| "语音合成失败。".to_string())?;
        let sample_rate = self.sample_rate();
        let samples = apply_speed_to_samples(audio.samples(), sample_rate, speed)?;
        crate::audio::write_wav_f32(out_path, sample_rate as u32, &samples)?;
        Ok(samples.len())
    }
}

/// 把参考音频归一化到目标采样率。
///
/// ZipVoice 的 Mel 频谱在跨采样率（如 48k→24k）时，sherpa C++ 重采样器可能抛
/// 异常，而 Rust 无法捕获 C++ 异常会直接 abort。统一到模型目标采样率后，
/// Mel 频谱重采样变为恒等变换，避免崩溃。同采样率时原样返回（零开销）。
fn normalize_reference(
    samples: &[f32],
    src_rate: i32,
    target_rate: i32,
) -> Result<(Vec<f32>, i32), String> {
    if src_rate == target_rate {
        return Ok((samples.to_vec(), src_rate));
    }
    let mut resampler = crate::audio::Resampler::new(src_rate, target_rate)?;
    let out = resampler.process(samples, true);
    Ok((out, target_rate))
}

/// 对合成输出应用语速：模型以 1.0 合成后，把样本重采样到 `sample_rate / speed`，
/// 再以 `sample_rate` 写回，从而改变时长（speed>1 更快、样本更少；speed<1 更慢、样本更多）。
///
/// 这是为了避免把 speed 传给 ZipVoice 模型：模型内部 `kept_frames =
/// num_frames(speed) - 参考帧数`，高语速 + 短文本时 `kept_frames≤0` 会抛 C++
/// 异常，而 Rust 无法捕获 C++ 异常会直接 abort。改用输出重采样后任何语速都不崩。
fn apply_speed_to_samples(
    samples: &[f32],
    sample_rate: i32,
    speed: f32,
) -> Result<Vec<f32>, String> {
    if speed <= 0.0 {
        return Err(format!("语速必须为正数，当前 {speed}"));
    }
    if (speed - 1.0).abs() < 1e-6 {
        return Ok(samples.to_vec());
    }
    let out_rate = (sample_rate as f32 / speed) as i32;
    let mut resampler = crate::audio::Resampler::new(sample_rate, out_rate)?;
    Ok(resampler.process(samples, true))
}

/// TTS 模型安装目录：`~/.zapmomo/models/<name>`。
pub fn user_model_dir() -> PathBuf {
    crate::kws::model::tts_user_model_dir()
}

/// 生成唯一的 TTS 输出 wav 路径：`~/.zapmomo/tts/tts-<毫秒时间戳>.wav`
pub fn default_output_path() -> PathBuf {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    crate::config::settings::get_tts_output_dir().join(format!("tts-{millis}.wav"))
}

/// 目标目录是否已装好 TTS 主模型。
pub fn is_installed(dir: &Path) -> bool {
    crate::kws::model::has_required_files(dir, &config::REQUIRED_FILES)
}

/// 安装 TTS 主模型到 `dest_dir`（默认 `~/.zapmomo/models/<name>`）。
///
/// 幂等：已安装且 `force` 为假时直接返回。下载过程中回调进度。
pub fn install_model_to(
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    crate::kws::model::install_asset_to(
        crate::kws::model::tts_asset(),
        dest_dir,
        force,
        on_progress,
        &config::REQUIRED_FILES,
    )
}

/// 安装 TTS 声码器到 `dest_dir`（独立发布的 vocos_24khz.onnx 单文件）。
///
/// 幂等：已安装且 `force` 为假时直接返回。
pub fn install_vocoder_to(
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    crate::kws::model::install_raw_file_to(
        crate::kws::model::tts_vocoder_asset(),
        &dest_dir.join(config::DEFAULT_VOCODER),
        force,
        on_progress,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_new_missing_model_errors() {
        // 用一个不存在的模型目录，TtsEngine::new 应报错提示下载模型
        let mut cfg = ResolvedTtsConfig::default();
        cfg.model_dir = PathBuf::from("/nonexistent/model");
        cfg.encoder = cfg.model_dir.join("encoder.int8.onnx");
        let err = TtsEngine::new(cfg.clone()).err().unwrap();
        assert!(err.contains("install-model"), "err: {err}");
    }

    #[test]
    #[ignore = "需要先运行 cargo run -- tts install-model 下载模型"]
    fn test_synthesize_produces_audio() {
        let cfg = config::resolve(None, None).unwrap();
        if !cfg.encoder.is_file() {
            eprintln!("跳过：模型未下载，请运行 cargo run -- tts install-model");
            return;
        }
        let engine = TtsEngine::new(cfg.clone()).unwrap();
        let samples = engine
            .synthesize(
                "你好，我是 ZapMomo。",
                1.0,
                &cfg.reference_wav,
                &cfg.reference_text,
            )
            .unwrap();
        assert!(!samples.is_empty(), "合成音频不应为空");
    }

    #[test]
    fn test_normalize_reference_identity_rate() {
        let samples = vec![0.1f32; 24000];
        let (out, rate) = normalize_reference(&samples, 24000, 24000).unwrap();
        assert_eq!(rate, 24000);
        assert_eq!(out.len(), 24000);
    }

    #[test]
    fn test_normalize_reference_resamples_48k_to_24k() {
        // 用户上传的 48k 参考音频 → 归一化到 24k（之前会导致 sherpa Mel 重采样崩溃）
        let samples = vec![0.1f32; 48000]; // 1 秒 @48k
        let (out, rate) = normalize_reference(&samples, 48000, 24000).unwrap();
        assert_eq!(rate, 24000);
        assert!(
            (out.len() as i64 - 24000).abs() <= 64,
            "resample len={}",
            out.len()
        );
    }

    #[test]
    fn test_normalize_reference_upsamples_16k_to_24k() {
        // 录音（16k）→ 归一化到 24k（上采样）
        let samples = vec![0.1f32; 16000]; // 1 秒 @16k
        let (out, rate) = normalize_reference(&samples, 16000, 24000).unwrap();
        assert_eq!(rate, 24000);
        assert!(
            (out.len() as i64 - 24000).abs() <= 64,
            "upsample len={}",
            out.len()
        );
    }

    #[test]
    fn test_apply_speed_identity() {
        let samples = vec![0.1f32; 24000];
        let out = apply_speed_to_samples(&samples, 24000, 1.0).unwrap();
        assert_eq!(out.len(), 24000);
    }

    #[test]
    fn test_apply_speed_faster_shortens() {
        // speed 1.3 → 样本数 ≈ 1/1.3（24k / 1.3 ≈ 18461 目标采样率）
        let samples = vec![0.1f32; 24000];
        let out = apply_speed_to_samples(&samples, 24000, 1.3).unwrap();
        assert!(
            (out.len() as i64 - 18461).abs() <= 64,
            "speed 1.3 len={}",
            out.len()
        );
    }

    #[test]
    fn test_apply_speed_slower_lengthens() {
        // speed 0.7 → 样本数 ≈ 1/0.7（24k / 0.7 ≈ 34285 目标采样率）
        let samples = vec![0.1f32; 24000];
        let out = apply_speed_to_samples(&samples, 24000, 0.7).unwrap();
        assert!(
            (out.len() as i64 - 34285).abs() <= 64,
            "speed 0.7 len={}",
            out.len()
        );
    }

    #[test]
    fn test_apply_speed_rejects_non_positive() {
        assert!(apply_speed_to_samples(&[0.0f32], 24000, 0.0).is_err());
        assert!(apply_speed_to_samples(&[0.0f32], 24000, -1.0).is_err());
    }
}
