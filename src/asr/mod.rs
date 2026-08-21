/// 流式语音识别（ASR）。
///
/// 使用 sherpa-onnx 的 `OnlineRecognizer`（streaming zipformer 中英双语模型）实现：
/// 离线转写 wav（`run_offline`）与实时麦克风转写（`run_realtime`）。
///
/// 设计上独立于 KWS，`run_realtime_with` 暴露 `should_stop` 供将来「KWS 唤醒后
/// 复用同一麦克风流」的联动场景使用。
pub mod config;
pub mod reaction;

use crate::audio::Resampler;
use config::ResolvedAsrConfig;
use sherpa_onnx::{
    OfflinePunctuation, OfflinePunctuationConfig, OnlineRecognizer, OnlineRecognizerConfig,
    OnlineStream, Wave,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::kws::model::{DownloadProgress, DownloadStage, ModelError, ProgressFn};
pub use crate::kws::reaction::ReactionOutcome;
pub use reaction::{AsrReaction, AsrResult, CollectAsrReaction, ConsoleAsrReaction};

/// 语音识别引擎。
///
/// 持有 `OnlineRecognizer`。所有方法接收 `&self`（sherpa 的 is_ready/decode/
/// get_result/reset 均只读），因此引擎可复用，不需要 `&mut`。
pub struct AsrEngine {
    recognizer: OnlineRecognizer,
    punctuation: Option<OfflinePunctuation>,
    cfg: ResolvedAsrConfig,
}

impl AsrEngine {
    /// 构造引擎，先校验所有模型文件存在。
    pub fn new(cfg: ResolvedAsrConfig) -> Result<Self, String> {
        let required = [
            ("encoder", &cfg.encoder),
            ("decoder", &cfg.decoder),
            ("joiner", &cfg.joiner),
            ("tokens", &cfg.tokens),
        ];
        for (name, path) in required {
            if !path.is_file() {
                return Err(format!(
                    "缺少模型文件 {name}: {}\n请运行 `zapmomo asr install-model` 下载模型。",
                    path.display()
                ));
            }
        }

        let mut c = OnlineRecognizerConfig::default();
        c.feat_config.sample_rate = cfg.sample_rate;
        c.model_config.transducer.encoder = Some(cfg.encoder.to_string_lossy().to_string());
        c.model_config.transducer.decoder = Some(cfg.decoder.to_string_lossy().to_string());
        c.model_config.transducer.joiner = Some(cfg.joiner.to_string_lossy().to_string());
        c.model_config.tokens = Some(cfg.tokens.to_string_lossy().to_string());
        c.model_config.provider = Some(cfg.provider.clone());
        c.model_config.num_threads = cfg.num_threads;
        c.model_config.debug = cfg.debug;
        // 热词（context graph）仅 modified_beam_search 支持；greedy 下会崩溃。
        // 配置了热词时自动切换解码方式。
        let use_hotwords = cfg
            .hotwords
            .as_deref()
            .is_some_and(|h| !h.trim().is_empty());
        c.decoding_method = Some(if use_hotwords {
            "modified_beam_search".to_string()
        } else {
            cfg.decoding_method.clone()
        });
        if use_hotwords {
            c.max_active_paths = 4;
        }
        c.enable_endpoint = cfg.enable_endpoint;
        c.rule1_min_trailing_silence = cfg.rule1_min_trailing_silence;
        c.rule2_min_trailing_silence = cfg.rule2_min_trailing_silence;
        c.rule3_min_utterance_length = cfg.rule3_min_utterance_length;
        c.blank_penalty = cfg.blank_penalty;

        let recognizer = OnlineRecognizer::create(&c)
            .ok_or_else(|| "无法创建 OnlineRecognizer，请检查模型文件与配置。".to_string())?;

        // 标点恢复：模型文件缺失或创建失败则降级为无标点，不影响 ASR 可用性。
        let punctuation = if cfg.enable_punctuation && cfg.punctuation_model.is_file() {
            let mut pc = OfflinePunctuationConfig::default();
            pc.model.ct_transformer = Some(cfg.punctuation_model.to_string_lossy().to_string());
            pc.model.num_threads = cfg.num_threads;
            pc.model.debug = cfg.debug;
            pc.model.provider = Some(cfg.provider.clone());
            OfflinePunctuation::create(&pc)
        } else {
            None
        };

        Ok(Self {
            recognizer,
            punctuation,
            cfg,
        })
    }

    pub fn config(&self) -> &ResolvedAsrConfig {
        &self.cfg
    }

    /// 创建一条识别流。`hotwords` 非空时对指定词提权。
    pub fn create_stream(&self, hotwords: Option<&str>) -> OnlineStream {
        match hotwords {
            Some(h) if !h.trim().is_empty() => self.recognizer.create_stream_with_hotwords(h),
            _ => self.recognizer.create_stream(),
        }
    }

    /// 喂入一帧音频（采样率 = `cfg.sample_rate`）。
    pub fn feed(&self, stream: &OnlineStream, samples: &[f32]) {
        stream.accept_waveform(self.cfg.sample_rate, samples);
    }

    /// 标记输入结束（离线路径 flush 出尾部结果）。
    pub fn finish(&self, stream: &OnlineStream) {
        stream.input_finished();
    }

    /// 识别循环：`while is_ready { decode; is_endpoint; get_result → reaction }`。
    ///
    /// 每次 decode 后先查端点检测（`is_endpoint`），据此把结果标记为最终并加标点、
    /// 清空流（`reset`）开始下一句。
    pub fn decode_loop(
        &self,
        stream: &OnlineStream,
        reaction: &mut dyn AsrReaction,
    ) -> ReactionOutcome {
        let mut outcome = ReactionOutcome::Continue;
        while self.recognizer.is_ready(stream) {
            self.recognizer.decode(stream);
            let is_final = self.recognizer.is_endpoint(stream);
            if let Some(r) = self.recognizer.get_result(stream) {
                let mut result = AsrResult::from(&r);
                result.is_final = is_final;
                let result = self.punctuate(result);
                if !result.text.is_empty() {
                    outcome = reaction.on_result(&result);
                    if outcome == ReactionOutcome::Stop {
                        break;
                    }
                }
            }
            if is_final {
                self.recognizer.reset(stream);
            }
        }
        outcome
    }

    /// 流是否还有足够的音频可解码（供离线 flush 循环使用）。
    pub fn is_ready(&self, stream: &OnlineStream) -> bool {
        self.recognizer.is_ready(stream)
    }

    /// 执行一步解码（供离线 flush 循环使用）。
    pub fn decode(&self, stream: &OnlineStream) {
        self.recognizer.decode(stream);
    }

    /// 获取当前识别结果（owned，供离线转写直接读取）。
    pub fn get_result(&self, stream: &OnlineStream) -> Option<AsrResult> {
        self.recognizer
            .get_result(stream)
            .map(|r| AsrResult::from(&r))
    }

    /// 对一段文本加标点（有标点模型则加，否则原样返回）。
    pub fn punctuate_text(&self, text: &str) -> String {
        if text.trim().is_empty() {
            return text.to_string();
        }
        if let Some(punct) = &self.punctuation
            && let Some(out) = punct.add_punctuation(text)
        {
            return out;
        }
        text.to_string()
    }

    /// 对最终结果加标点（部分结果不加，避免实时闪烁）。
    fn punctuate(&self, result: AsrResult) -> AsrResult {
        if !result.is_final || result.text.trim().is_empty() {
            return result;
        }
        AsrResult {
            text: self.punctuate_text(&result.text),
            ..result
        }
    }
}

/// ASR 模型安装目录：`~/.zapmomo/models/<name>`。
pub fn user_model_dir() -> PathBuf {
    crate::kws::model::asr_user_model_dir()
}

/// 目标目录是否已装好 ASR 模型（探测式：按目录内容探测四件套，模型无关）。
pub fn is_installed(dir: &Path) -> bool {
    config::asr_files_present(dir)
}

/// 安装 ASR 模型到 `dest_dir`（默认 `~/.zapmomo/models/<name>`）。
///
/// 幂等：已安装且 `force` 为假时直接返回。下载过程中回调进度。
pub fn install_model_to(
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    crate::kws::model::install_asset_to(
        crate::kws::model::asr_asset(),
        dest_dir,
        force,
        on_progress,
        &config::REQUIRED_FILES,
    )
}

/// 标点模型安装目录：`~/.zapmomo/models/<标点模型名>`。
pub fn punctuation_user_model_dir() -> PathBuf {
    crate::kws::model::punctuation_user_model_dir()
}

/// 安装标点模型到 `dest_dir`（默认 `~/.zapmomo/models/<标点模型名>`）。
///
/// 幂等：已安装且 `force` 为假时直接返回。下载过程中回调进度。
pub fn install_punctuation_model_to(
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    crate::kws::model::install_punctuation_model_to(
        dest_dir,
        force,
        on_progress,
        &config::PUNCT_REQUIRED_FILES,
    )
}

/// 离线转写 wav 文件，返回带标点的转写文本（不依赖麦克风）。
///
/// 整段转写（不做端点断句，避免静音触发多次 reset 丢失开头文本）。
/// 供 CLI 离线验证与「参考音频自动转写」复用。
pub fn transcribe_wav(cfg: &ResolvedAsrConfig, wav: &Path) -> Result<String, String> {
    let engine = AsrEngine::new(cfg.clone())?;
    let stream = engine.create_stream(cfg.hotwords.as_deref());
    let wave = Wave::read(&wav.to_string_lossy())
        .ok_or_else(|| format!("无法读取 wav: {}", wav.display()))?;

    // 若 wav 采样率 != 模型采样率，先重采样（test_wavs 是 16k，一般直接走 else）
    if wave.sample_rate() != cfg.sample_rate {
        let mut rs = Resampler::new(wave.sample_rate(), cfg.sample_rate)?;
        let out = rs.process(wave.samples(), true);
        engine.feed(&stream, &out);
    } else {
        engine.feed(&stream, wave.samples());
    }
    // 尾部补 0.5s 静音，让模型 flush 出最后一个结果
    let tail = vec![0.0f32; (cfg.sample_rate as usize) / 2];
    engine.feed(&stream, &tail);
    engine.finish(&stream);

    while engine.is_ready(&stream) {
        engine.decode(&stream);
    }
    let text = engine
        .get_result(&stream)
        .map(|r| engine.punctuate_text(&r.text))
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err("未能识别出有效文本，请换一段更清晰的参考音频".to_string());
    }
    Ok(text)
}

/// 离线转写 wav 文件并打印结果（不依赖麦克风）。
///
/// 用于验证模型与整条链路：对模型自带 `test_wavs/*.wav` 应输出对应文本。
pub fn run_offline(cfg: &ResolvedAsrConfig, wav: &Path) -> Result<(), String> {
    let text = transcribe_wav(cfg, wav)?;
    println!("[识别] {text}");
    Ok(())
}

/// 实时监听麦克风并转写（默认反应 + 不可取消，供 CLI 使用）。
///
/// 等价于 `run_realtime_with(cfg, device, duration, ConsoleAsrReaction, None)`。
pub fn run_realtime(
    cfg: &ResolvedAsrConfig,
    device: Option<&str>,
    duration: Option<u64>,
) -> Result<(), String> {
    let mut reaction = ConsoleAsrReaction;
    run_realtime_with(cfg, device, duration, &mut reaction, None)
}

/// 实时监听麦克风并转写。
///
/// 线程模型：cpal 采集在系统音频线程，经 `mpsc` 送到调用线程；
/// 调用线程内做重采样 + 识别循环（sherpa 类型不跨线程）。
///
/// `reaction` 为可插拔识别反应（GUI 可实现自己的 `AsrReaction` 发事件给前端）；
/// `should_stop` 为非空时，每次迭代检查该标志，置位则干净退出（返回 `Ok(())`），
/// 供桌面 GUI 的「停止识别」使用。
pub fn run_realtime_with(
    cfg: &ResolvedAsrConfig,
    device: Option<&str>,
    duration: Option<u64>,
    reaction: &mut dyn AsrReaction,
    should_stop: Option<&AtomicBool>,
) -> Result<(), String> {
    let engine = AsrEngine::new(cfg.clone())?;
    let stream = engine.create_stream(cfg.hotwords.as_deref());

    let mut mic = crate::audio::start_capture(device)?;
    let mut resampler = Resampler::new(mic.device_sample_rate() as i32, cfg.sample_rate)?;
    let mut pending: Vec<f32> = Vec::with_capacity(cfg.chunk_size * 2);
    let start = std::time::Instant::now();
    let deadline = duration.map(|secs| start + std::time::Duration::from_secs(secs));

    // should_stop 标志语义为 `running`（true = 正在识别）；因此「应停止」= 标志为 false。
    // CLI 传 None 时恒为 false（不主动停止，由 Ctrl-C / --duration 控制）。
    let stop_requested = || should_stop.is_some_and(|f| !f.load(Ordering::Relaxed));

    println!("开始语音识别... (Ctrl-C 退出; --duration 可限制时长)");
    let mut chunks_received: u64 = 0;
    let mut process =
        |raw: Vec<f32>, engine: &AsrEngine, stream: &OnlineStream| -> Result<bool, String> {
            let out = resampler.process(&raw, false);
            pending.extend_from_slice(&out);
            while pending.len() >= cfg.chunk_size {
                let chunk: Vec<f32> = pending.drain(..cfg.chunk_size).collect();
                engine.feed(stream, &chunk);
                if engine.decode_loop(stream, reaction) == ReactionOutcome::Stop {
                    return Ok(true); // 应停止
                }
                if stop_requested() {
                    return Ok(true); // GUI 请求停止
                }
            }
            Ok(false)
        };

    loop {
        if stop_requested() {
            tracing::warn!("ASR 识别退出：收到停止请求（共收到 {chunks_received} 块）");
            break;
        }
        let raw = if let Some(dl) = deadline {
            if std::time::Instant::now() >= dl {
                break;
            }
            let timeout = dl
                .saturating_duration_since(std::time::Instant::now())
                .min(std::time::Duration::from_millis(500));
            match mic.recv_chunk_timeout(timeout) {
                Ok(raw) => Some(raw),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::warn!("ASR 识别退出：麦克风通道断开（共收到 {chunks_received} 块）");
                    break;
                }
            }
        } else {
            mic.recv_chunk()
        };

        let Some(raw) = raw else {
            tracing::warn!("ASR 识别退出：麦克风返回 None（共收到 {chunks_received} 块）");
            break;
        };
        chunks_received += 1;
        if process(raw, &engine, &stream)? {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_engine_new_missing_model_errors() {
        // 用一个不存在的模型目录，AsrEngine::new 应报错提示下载模型
        let mut cfg = ResolvedAsrConfig::default();
        cfg.model_dir = PathBuf::from("/nonexistent/model");
        cfg.encoder = cfg.model_dir.join("encoder.onnx");
        let err = AsrEngine::new(cfg.clone()).err().unwrap();
        assert!(err.contains("install-model"), "err: {err}");
    }

    #[test]
    #[ignore = "需要先运行 cargo run -- asr install-model 下载模型"]
    fn test_offline_transcribes_test_wav() {
        let cfg = config::resolve(None, None).unwrap();
        if !cfg.encoder.is_file() {
            eprintln!("跳过：模型未下载，请运行 cargo run -- asr install-model");
            return;
        }
        let engine = AsrEngine::new(cfg.clone()).unwrap();
        let stream = engine.create_stream(cfg.hotwords.as_deref());
        let wave = Wave::read(&cfg.model_dir.join("test_wavs/0.wav").to_string_lossy()).unwrap();
        engine.feed(&stream, wave.samples());
        engine.feed(&stream, &vec![0.0; (cfg.sample_rate as usize) / 2]);
        engine.finish(&stream);

        let mut collect = CollectAsrReaction::new();
        engine.decode_loop(&stream, &mut collect);
        assert!(
            collect.results.iter().any(|r| !r.text.trim().is_empty()),
            "应转写出非空文本，实际: {:?}",
            collect
                .results
                .iter()
                .map(|r| r.text.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "需要先运行 cargo run -- asr install-model 下载模型（含标点）"]
    fn test_offline_punctuates_final_text() {
        let cfg = config::resolve(None, None).unwrap();
        if !cfg.encoder.is_file() {
            eprintln!("跳过：模型未下载");
            return;
        }
        let engine = AsrEngine::new(cfg.clone()).unwrap();
        let stream = engine.create_stream(cfg.hotwords.as_deref());
        let wave = Wave::read(&cfg.model_dir.join("test_wavs/0.wav").to_string_lossy()).unwrap();
        engine.feed(&stream, wave.samples());
        engine.feed(&stream, &vec![0.0; cfg.sample_rate as usize * 3]);
        engine.finish(&stream);

        let mut collect = CollectAsrReaction::new();
        engine.decode_loop(&stream, &mut collect);
        let finals: Vec<&str> = collect
            .results
            .iter()
            .filter(|r| r.is_final)
            .map(|r| r.text.as_str())
            .collect();
        if engine.punctuation.is_some() {
            assert!(
                finals.iter().any(|t| {
                    t.contains('。') || t.contains('，') || t.contains('.') || t.contains(',')
                }),
                "标点模型就绪时应含标点，实际: {finals:?}"
            );
        } else {
            eprintln!("标点模型未就绪，跳过标点断言");
        }
    }
}
