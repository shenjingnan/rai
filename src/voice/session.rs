/// 语音会话编排核心（`VoiceSession`）。
///
/// 把 KWS → ASR → LLM → TTS 串成一条**唤醒门控 + 句级流式 + 唤醒词打断**的对话链路：
///
/// ```text
/// Idle --Start--> Armed --唤醒词--> Listening --ASR is_final--> Thinking --首句入队--> Speaking
///   ▲                                             │                                 │
///   │                     ReplyFinished（播完回听）│                                 │
///   └──────────────────── Armed ◄───BargeIn────────┴─────────────Thinking | Speaking──┘
/// ```
///
/// `Armed`（待唤醒）是 KWS 门控：命中唤醒词才进入 `Listening`（ASR 识别），
/// 否则不消费用户话语。
///
/// 线程模型：编排循环在**调用线程**运行，持有全部 sherpa 引擎/流与 rodio 播放器
/// （`Sink`/`Player` 不跨线程）。唯一后台线程是 [`SynthHandle`] 的 TTS 合成线程。
/// 整个会话只开一次麦克风（[`MicLoop`]），按状态把 chunk 喂给 KWS 流（待唤醒/
/// 播报/思考）或 ASR 流（聆听）——KWS/ASR 各自 `start_capture` 会在同设备冲突。
///
/// 打断序列：KWS 命中 → `llm.cancel()` + `speaker.stop()` + `synth.cancel_all()` +
/// `current_gen += 1`（作废在途合成结果）+ 回 Armed 前 `skip_for` 丢回声尾巴。
use crate::asr::{AsrEngine, AsrReaction, AsrResult};
use crate::kws::{KwsEngine, KwsResult, Reaction, ReactionOutcome};
use crate::llm::LlmEngine;
use crate::llm::LlmEvent;
use crate::llm::types::{ChatMessage, ChatRole, InputItem};
use crate::tts::TtsEngine;
use crate::voice::config::ResolvedSessionConfig;
use crate::voice::events::{ErrorKind, StoppedReason, VoiceEvent};
use crate::voice::listen::{MicEvent, MicLoop};
use crate::voice::player::AudioPlayer;
use crate::voice::splitter::SentenceSplitter;
use crate::voice::state::{SessionEvent, SessionState};
use crate::voice::synthesizer::{SynthHandle, SynthResult};
use sherpa_onnx::OnlineStream;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::Duration;

/// 编排循环单次轮询麦克风的最长等待（块间隔远小于此，不影响实时性）。
const MIC_POLL: Duration = Duration::from_millis(100);
/// 打断后回听前跳过的音频时长（丢弃回声尾巴，避免把上一条回答喂给 ASR）。
const SKIP_AFTER_BARGE_IN: Duration = Duration::from_millis(300);
/// LLM 模型加载超时（首次加载大模型较慢）。
const LLM_LOAD_TIMEOUT: Duration = Duration::from_secs(180);

/// 语音会话编排器。
pub struct VoiceSession {
    cfg: ResolvedSessionConfig,
    kws: KwsEngine,
    asr: AsrEngine,
    llm: Arc<LlmEngine>,
    /// 会话订阅的 LLM 事件流（与 GUI 的 forward 订阅互不抢事件）
    llm_rx: Receiver<LlmEvent>,
    speaker: Box<dyn AudioPlayer>,
    synth: SynthHandle,
    mic: MicLoop,
    asr_stream: OnlineStream,
    kws_stream: OnlineStream,

    state: SessionState,
    /// true = 运行中（CLI 用 Ctrl-C / Tauri 用 stop 命令置 false 优雅退出）
    pub running: Arc<AtomicBool>,
    /// 打断标志（KWS reaction 在 Thinking/Speaking 期间置位，编排循环每轮检查）
    barge_in: Arc<AtomicBool>,

    history: Vec<InputItem>,
    reply: ReplyAccumulator,
    reply_done: bool,
    current_gen: u64,
    synth_enqueued: u64,
    synth_consumed: u64,
    turns: u32,
    first_sentence: bool,
    /// 与合成入队顺序对应的句子文本（播放时弹出打印 `[播放]`，打断时清空）
    pending_speech: VecDeque<String>,
    /// 事件输出（CLI 用 stdout sink / Tauri 用 app.emit sink）
    emit: Box<dyn Fn(VoiceEvent) + Send>,
    /// Listening 阶段连续静音累计（秒），达到 `asr_max_trailing_silence` 判定说完
    silence_accum: f32,
    /// WaitingSpeech 阶段连续说话块计数（防瞬时噪音误触发）
    speech_hits: u32,
    /// WaitingSpeech 阶段等待时长累计（秒），超时回待唤醒
    speech_wait_accum: f32,
    /// 欢迎语是否已播放（合成失败也置位，跳过欢迎不卡流程）
    welcome_played: bool,
}

impl VoiceSession {
    /// 构造会话（CLI 默认：stdout sink + 自建 running 标志 + 自建 LLM 引擎）。
    pub fn new(cfg: ResolvedSessionConfig) -> Result<Self, String> {
        Self::new_with_parts(
            cfg,
            Box::new(crate::voice::events::cli_sink),
            Arc::new(AtomicBool::new(true)),
            None,
        )
    }

    /// 构造会话（供 Tauri 等宿主注入事件 sink 与外部停止标志）。
    ///
    /// 校验并创建各引擎 + 打开麦克风与音频输出，任一失败返回带安装提示的错误。
    ///
    /// - `llm`：`Some` 复用宿主（Tauri `LlmState`）共享的引擎（**只加载一份模型**）；
    ///   `None` 自建（CLI `voice run`）。
    /// - **说完判定由 session 内 RMS 静音统一控制**，因此这里强制禁用 sherpa endpoint
    ///   （避免其 rule1/rule2 的 1.2s/2.4s 尾静音在期望的 3s 前提前断句并 reset 流）。
    pub fn new_with_parts(
        mut cfg: ResolvedSessionConfig,
        emit: Box<dyn Fn(VoiceEvent) + Send>,
        running: Arc<AtomicBool>,
        llm: Option<Arc<LlmEngine>>,
    ) -> Result<Self, String> {
        cfg.asr.enable_endpoint = false;
        let kws = KwsEngine::new(cfg.kws.clone())?;
        let asr = AsrEngine::new(cfg.asr.clone())?;
        let llm = match llm {
            Some(e) => e,
            None => Arc::new(LlmEngine::new(cfg.llm.clone()).map_err(|e| e.to_string())?),
        };
        let llm_rx = llm.subscribe();
        let tts = TtsEngine::new(cfg.tts.clone())?;
        // 参考音色：自定义音色 > 内置音色 id > 配置默认
        let (ref_wav, ref_text) =
            crate::tts::voice::resolve_reference(&cfg.tts, cfg.voice_id.as_deref(), None, None)?;
        let synth = SynthHandle::new(tts, ref_wav, ref_text, cfg.speed);
        let mic = MicLoop::new(
            cfg.mic_device.as_deref(),
            cfg.asr.sample_rate,
            cfg.asr.chunk_size,
        )?;
        let speaker = Box::new(crate::voice::player::Speaker::try_new()?);
        let kws_stream = Self::make_kws_stream(&kws, &cfg)?;
        let asr_stream = asr.create_stream(cfg.asr.hotwords.as_deref());

        Ok(Self {
            cfg,
            kws,
            asr,
            llm,
            llm_rx,
            speaker,
            synth,
            mic,
            asr_stream,
            kws_stream,
            state: SessionState::Idle,
            running,
            barge_in: Arc::new(AtomicBool::new(false)),
            history: Vec::new(),
            reply: ReplyAccumulator::new(),
            reply_done: false,
            current_gen: 0,
            synth_enqueued: 0,
            synth_consumed: 0,
            turns: 0,
            first_sentence: false,
            pending_speech: VecDeque::new(),
            emit,
            silence_accum: 0.0,
            speech_hits: 0,
            speech_wait_accum: 0.0,
            welcome_played: false,
        })
    }

    /// 构造 KWS 流（自定义唤醒词需先编码为 token）。
    fn make_kws_stream(
        kws: &KwsEngine,
        cfg: &ResolvedSessionConfig,
    ) -> Result<OnlineStream, String> {
        match cfg.keywords.as_deref() {
            Some(k) => {
                let encoded = crate::kws::token::encode_custom_keywords(k, &cfg.kws.tokens)?;
                Ok(kws.create_stream_with_keywords(&encoded))
            }
            None => Ok(kws.create_stream()),
        }
    }

    /// 运行会话主循环（阻塞直到停止）。
    pub fn run(&mut self) -> Result<(), String> {
        // 共享引擎已加载（Tauri LlmState）则跳过；CLI 自建引擎未加载则阻塞加载
        if !self.llm.is_ready() {
            self.llm.load_blocking(LLM_LOAD_TIMEOUT)?;
        }
        (self.emit)(VoiceEvent::Started);
        self.set_state(SessionEvent::Start)?;

        loop {
            if !self.running.load(Ordering::Relaxed) {
                break;
            }
            // 打断优先于状态推进
            if self.barge_in.load(Ordering::Relaxed)
                && matches!(self.state, SessionState::Thinking | SessionState::Speaking)
            {
                self.do_barge_in();
                continue;
            }
            match self.state {
                SessionState::Idle => break,
                SessionState::Armed => self.step_armed()?,
                SessionState::Greeting => self.step_greeting()?,
                SessionState::WaitingSpeech => self.step_waiting_speech()?,
                SessionState::Listening => self.step_listening()?,
                SessionState::Thinking => self.step_thinking()?,
                SessionState::Speaking => self.step_speaking()?,
            }
        }
        (self.emit)(VoiceEvent::Stopped {
            reason: StoppedReason::Manual,
            turns: self.turns,
        });
        Ok(())
    }

    /// 状态迁移 + 进入 Listening 时重建 ASR 流（丢弃上轮累积的识别状态）。
    fn set_state(&mut self, ev: SessionEvent) -> Result<(), String> {
        let next = crate::voice::state::transition(self.state, ev)?;
        self.state = next;
        if next == SessionState::Listening {
            self.asr_stream = self.asr.create_stream(self.cfg.asr.hotwords.as_deref());
        }
        (self.emit)(VoiceEvent::State { state: next });
        Ok(())
    }

    /// Armed：待唤醒，喂 KWS 检测唤醒词；命中 → 合成欢迎语并切到 Greeting（播欢迎语）。
    fn step_armed(&mut self) -> Result<(), String> {
        let chunk = match self.mic.next(MIC_POLL)? {
            MicEvent::Chunk(c) => c,
            MicEvent::Timeout => return Ok(()),
            MicEvent::Disconnected => return Err("麦克风已断开".to_string()),
        };
        self.kws.feed(&self.kws_stream, &chunk);
        let mut reaction = WakeReaction::default();
        let _ = self.kws.detect(&self.kws_stream, &mut reaction);
        if let Some(keyword) = reaction.keyword {
            (self.emit)(VoiceEvent::Wake { keyword });
            // 唤醒 → 合成并播放欢迎语（复用 SynthHandle；进入 Greeting 等结果）
            self.synth.clear_cancel();
            self.synth
                .enqueue(self.cfg.welcome_text.clone(), self.current_gen);
            self.welcome_played = false;
            self.set_state(SessionEvent::KeywordDetected)?; // → Greeting
        }
        Ok(())
    }

    /// Greeting：播放欢迎语音（复用 SynthHandle 合成，期间不喂麦克风防回声）。
    fn step_greeting(&mut self) -> Result<(), String> {
        // 消费麦克风（丢弃，不喂 ASR/KWS），保持采集活跃、避免回声被拾入
        match self.mic.next(MIC_POLL)? {
            MicEvent::Chunk(_) | MicEvent::Timeout => {}
            MicEvent::Disconnected => return Err("麦克风已断开".to_string()),
        }
        // 等欢迎语合成结果并播放
        while let Some(result) = self.synth.try_recv() {
            match result {
                SynthResult::Done {
                    gen_id,
                    samples,
                    sample_rate,
                } => {
                    if gen_id == self.current_gen {
                        self.speaker.play(samples, sample_rate);
                        self.welcome_played = true;
                    }
                }
                SynthResult::Error { message, .. } => {
                    tracing::warn!("欢迎语合成失败，跳过: {message}");
                    self.welcome_played = true; // 跳过欢迎，不卡流程
                }
            }
        }
        // 欢迎语播放完（或合成失败跳过）→ 进 WaitingSpeech
        if self.welcome_played && self.speaker.drained() {
            self.mic.skip_for(SKIP_AFTER_BARGE_IN); // 丢回声尾巴
            self.set_state(SessionEvent::WelcomeDone)?;
        }
        Ok(())
    }

    /// WaitingSpeech：RMS 门控，等用户真正说话（连续超阈值块）才进 ASR。
    fn step_waiting_speech(&mut self) -> Result<(), String> {
        let chunk = match self.mic.next(MIC_POLL)? {
            MicEvent::Chunk(c) => Some(c),
            MicEvent::Timeout => None,
            MicEvent::Disconnected => return Err("麦克风已断开".to_string()),
        };
        if let Some(chunk) = chunk {
            if chunk_rms(&chunk) > self.cfg.vad_silence_threshold {
                self.speech_hits += 1;
                if self.speech_hits >= 2 {
                    self.speech_hits = 0;
                    self.set_state(SessionEvent::SpeechDetected)?; // → Listening
                }
            } else {
                self.speech_hits = 0;
            }
        }
        // 等待超时（无人说话）→ 回待唤醒
        self.speech_wait_accum += MIC_POLL.as_secs_f32();
        if self.speech_wait_accum >= self.cfg.welcome_wait_timeout {
            self.speech_wait_accum = 0.0;
            self.speech_hits = 0;
            self.set_state(SessionEvent::WaitTimeout)?;
        }
        Ok(())
    }

    /// Listening：喂 ASR（流式字幕）+ RMS 静音累计；连续静音达 `asr_max_trailing_silence`
    /// 判定说完（强制 flush 取最终文本）→ 入历史 → 发起 LLM 生成。
    fn step_listening(&mut self) -> Result<(), String> {
        let chunk = match self.mic.next(MIC_POLL)? {
            MicEvent::Chunk(c) => c,
            MicEvent::Timeout => return Ok(()),
            MicEvent::Disconnected => return Err("麦克风已断开".to_string()),
        };
        self.asr.feed(&self.asr_stream, &chunk);
        let mut collector = AsrCollector::default();
        let _ = self.asr.decode_loop(&self.asr_stream, &mut collector);
        // 流式字幕：部分识别结果逐步刷新（覆盖同一行）
        if !collector.partial.is_empty() {
            (self.emit)(VoiceEvent::Transcript {
                text: collector.partial.clone(),
                is_final: false,
            });
        }
        // RMS 静音累计：有语音则重置，否则累加当前块时长
        let chunk_secs = self.cfg.asr.chunk_size as f32 / self.cfg.asr.sample_rate as f32;
        if chunk_rms(&chunk) > self.cfg.vad_silence_threshold {
            self.silence_accum = 0.0;
        } else {
            self.silence_accum += chunk_secs;
        }
        // 结束判定：sherpa is_final（兜底）或连续静音达到 asr_max_trailing_silence
        if let Some(text) = collector.final_text {
            self.handle_user_final(text)?;
        } else if self.silence_accum >= self.cfg.asr_max_trailing_silence {
            let text = self.force_finalize_asr();
            self.handle_user_final(text)?;
        }
        Ok(())
    }

    /// 处理一句最终用户文本：非空 → 入历史并发起 LLM；空（没真说话/识别失败）→ 回待唤醒。
    fn handle_user_final(&mut self, text: String) -> Result<(), String> {
        let text = text.trim().to_string();
        self.silence_accum = 0.0;
        if text.is_empty() {
            self.set_state(SessionEvent::WaitTimeout)?;
            return Ok(());
        }
        (self.emit)(VoiceEvent::Transcript {
            text: text.clone(),
            is_final: true,
        });
        self.turns += 1;
        self.history
            .push(InputItem::Message(ChatMessage::new(ChatRole::User, text)));
        truncate_history(&mut self.history, self.cfg.history_max);
        self.start_reply();
        let input = build_llm_input(&self.cfg.llm.system_prompt, &self.history);
        match self.llm.generate(input, self.cfg.llm.params.clone()) {
            Ok(()) => {}
            Err(e) => {
                // 生成互斥（GUI 正在生成）或 worker 退出 → 转错误事件，回待唤醒（不崩溃）
                (self.emit)(VoiceEvent::Error {
                    kind: ErrorKind::Llm,
                    message: e.to_string(),
                });
                self.set_state(SessionEvent::WaitTimeout)?;
                return Ok(());
            }
        }
        self.set_state(SessionEvent::UserUtteranceFinal)
    }

    /// 强制结束当前句：补尾部静音 + `input_finished` + drain，取最终识别文本
    /// （模式同 `asr::transcribe_wav`）。
    fn force_finalize_asr(&mut self) -> String {
        let tail = vec![0.0f32; (self.cfg.asr.sample_rate as usize) / 2];
        self.asr.feed(&self.asr_stream, &tail);
        self.asr.finish(&self.asr_stream);
        while self.asr.is_ready(&self.asr_stream) {
            self.asr.decode(&self.asr_stream);
        }
        self.asr
            .get_result(&self.asr_stream)
            .map(|r| self.asr.punctuate_text(&r.text))
            .unwrap_or_default()
    }

    /// 进入一轮新生成前的重置（gen 递增、清空上一轮回复状态、复位合成取消）。
    fn start_reply(&mut self) {
        self.current_gen += 1;
        self.reply = ReplyAccumulator::new();
        self.reply_done = false;
        self.first_sentence = false;
        self.synth_enqueued = 0;
        self.synth_consumed = 0;
        self.pending_speech.clear();
        self.synth.clear_cancel();
    }

    /// 把一句文本入队合成，并记录其文本（播放时弹出打印）。
    fn enqueue_sentence(&mut self, sentence: String) {
        (self.emit)(VoiceEvent::ReplySentence {
            sentence: sentence.clone(),
        });
        self.pending_speech.push_back(sentence.clone());
        self.synth.enqueue(sentence, self.current_gen);
        self.synth_enqueued += 1;
    }

    /// Thinking：喂 KWS（打断监听）+ 轮询 LLM 事件（流式打印 token），把切句入队合成。
    fn step_thinking(&mut self) -> Result<(), String> {
        self.listen_barge_in()?;
        // 轮询本会话订阅的 LLM 事件流（广播：与 GUI 的 forward 订阅互不抢事件）
        loop {
            let ev = match self.llm_rx.try_recv() {
                Ok(ev) => ev,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            };
            match ev {
                LlmEvent::Token(delta) => {
                    let (visible, sentences) = self.reply.push_token(&delta.text);
                    // 只输出可见文本（思考块被过滤，不上屏）
                    if !visible.is_empty() {
                        (self.emit)(VoiceEvent::Token { delta: visible });
                    }
                    for s in sentences {
                        self.enqueue_sentence(s);
                        if !self.first_sentence {
                            self.first_sentence = true;
                            self.set_state(SessionEvent::FirstSentenceEnqueued)?;
                        }
                    }
                }
                LlmEvent::Finished(reason) => {
                    self.reply_done = true;
                    if let Some(tail) = self.reply.finish() {
                        self.enqueue_sentence(tail);
                    }
                    if let Some(reply) = self.reply.take_text() {
                        self.history.push(InputItem::Message(ChatMessage::new(
                            ChatRole::Assistant,
                            reply,
                        )));
                        truncate_history(&mut self.history, self.cfg.history_max);
                    }
                    (self.emit)(VoiceEvent::ReplyFinished {
                        reason: format!("{reason:?}"),
                    });
                }
                LlmEvent::Error(e) => {
                    self.reply_done = true;
                    (self.emit)(VoiceEvent::Error {
                        kind: ErrorKind::Llm,
                        message: e,
                    });
                }
                LlmEvent::Status { .. } => {}
            }
        }
        // 未切出任何句子（空回复/立即出错）→ 直接回听
        if self.reply_done && self.synth_enqueued == 0 {
            self.finish_reply()?;
        }
        Ok(())
    }

    /// Speaking：喂 KWS（打断监听）+ 把合成结果按序交给播放器，播完回听。
    fn step_speaking(&mut self) -> Result<(), String> {
        self.listen_barge_in()?;
        while let Some(result) = self.synth.try_recv() {
            self.synth_consumed += 1;
            match result {
                SynthResult::Done {
                    gen_id,
                    samples,
                    sample_rate,
                } => {
                    if gen_id == self.current_gen {
                        // 弹出与入队顺序对应的句子文本（播放状态展示）
                        let text = self.pending_speech.pop_front().unwrap_or_default();
                        (self.emit)(VoiceEvent::PlaySentence {
                            sentence: text.clone(),
                        });
                        self.speaker.play(samples, sample_rate);
                    }
                    // 过期结果（打断后迟到）直接丢弃
                }
                SynthResult::Error { gen_id, message } => {
                    if gen_id == self.current_gen {
                        self.pending_speech.pop_front();
                        (self.emit)(VoiceEvent::Error {
                            kind: ErrorKind::Synth,
                            message,
                        });
                    }
                }
            }
        }
        // 回复生成完 + 合成全消费 + 播放队列播完 → 回听
        if self.reply_done && self.synth_enqueued == self.synth_consumed && self.speaker.drained() {
            self.finish_reply()?;
        }
        Ok(())
    }

    /// Thinking/Speaking 期间喂麦克风给 KWS（打断词/唤醒词监听）。
    fn listen_barge_in(&mut self) -> Result<(), String> {
        let chunk = match self.mic.next(MIC_POLL)? {
            MicEvent::Chunk(c) => c,
            MicEvent::Timeout => return Ok(()),
            MicEvent::Disconnected => return Err("麦克风已断开".to_string()),
        };
        self.kws.feed(&self.kws_stream, &chunk);
        let mut reaction = BargeInReaction {
            flag: &self.barge_in,
        };
        let _ = self.kws.detect(&self.kws_stream, &mut reaction);
        Ok(())
    }

    /// 回复播完（或无内容可播）→ 回 Armed（待唤醒）；已达 max_turns 则结束会话。
    fn finish_reply(&mut self) -> Result<(), String> {
        if let Some(max) = self.cfg.max_turns
            && self.turns >= max
        {
            self.running.store(false, Ordering::Relaxed);
            (self.emit)(VoiceEvent::Stopped {
                reason: StoppedReason::MaxTurns { max },
                turns: self.turns,
            });
            return Ok(());
        }
        self.set_state(SessionEvent::ReplyFinished)
    }

    /// 打断序列：取消 LLM、停播、清合成、作废在途结果、回听并丢回声尾巴。
    fn do_barge_in(&mut self) {
        (self.emit)(VoiceEvent::BargeIn);
        self.llm.cancel();
        self.current_gen += 1;
        self.speaker.stop();
        self.reply = ReplyAccumulator::new();
        self.reply_done = false;
        self.first_sentence = false;
        self.pending_speech.clear();
        self.synth.cancel_all();
        self.synth_enqueued = 0;
        self.synth_consumed = 0;
        self.barge_in.store(false, Ordering::Relaxed);
        if let Err(e) = self.set_state(SessionEvent::BargeIn) {
            (self.emit)(VoiceEvent::Error {
                kind: ErrorKind::BargeIn,
                message: e,
            });
        }
        self.mic.skip_for(SKIP_AFTER_BARGE_IN);
    }
}

/// 一句话的回复累积：过滤思考块 → 拼接可见文本 + 切句（供合成入队）。
///
/// 独立成可测结构：`push_token` 返回（可见文本, 本次切出的句子）；`finish` 冲刷
/// 残余句；`take_text` 取完整可见回复（入历史后即丢弃，打断时直接 new 一个丢弃）。
#[derive(Default)]
pub struct ReplyAccumulator {
    text: String,
    splitter: SentenceSplitter,
    filter: ThinkingFilter,
}

impl ReplyAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 吸收一段 token 增量：过滤思考块后返回（可见文本, 切出的完整句子）。
    ///
    /// 思考块（`<think>...</think>`）内容**不进可见文本**，因此不会被切句合成、
    /// 不会入历史、不会上屏。
    pub fn push_token(&mut self, delta: &str) -> (String, Vec<String>) {
        let visible = self.filter.feed(delta);
        if visible.is_empty() {
            return (String::new(), Vec::new());
        }
        self.text.push_str(&visible);
        let sentences = self.splitter.push(&visible);
        (visible, sentences)
    }

    /// 生成结束：冲刷过滤器残余 → 切句 → 返回最后一句话（`None` = 无残余可合成）。
    pub fn finish(&mut self) -> Option<String> {
        let tail = self.filter.finish();
        if !tail.is_empty() {
            self.text.push_str(&tail);
            self.splitter.push(&tail);
        }
        let rest = self.splitter.finish();
        if rest.is_empty() { None } else { Some(rest) }
    }

    /// 完整可见回复文本（trim 后；空返回 `None`）。
    pub fn take_text(&mut self) -> Option<String> {
        let t = self.text.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    }
}

/// 思考块标签。`<think>` 块内的内容（思考过程）不送 TTS、不入历史、不上屏。
const THINK_OPEN: &str = "<think>";
const THINK_CLOSE: &str = "</think>";

/// 流式思考块过滤器：丢弃 `<think>...</think>` 之间的内容。
///
/// 流式 token 可能把标签拆成多段（如 `<th` + `ink>`），因此保留尾部若干字节
/// （最长标签长度 - 1）等待补全；未闭合的思考块在 `finish` 时整体丢弃。
#[derive(Default)]
struct ThinkingFilter {
    in_think: bool,
    buffer: String,
}

impl ThinkingFilter {
    /// 吸收增量，返回过滤掉思考块后的可见文本。
    fn feed(&mut self, delta: &str) -> String {
        self.buffer.push_str(delta);
        let mut out = String::new();
        loop {
            if self.in_think {
                match self.buffer.find(THINK_CLOSE) {
                    Some(pos) => {
                        self.buffer.drain(..pos + THINK_CLOSE.len());
                        self.in_think = false;
                        // 继续处理 </think> 之后的内容
                    }
                    None => {
                        // 思考块内：只保留可能是 `</think>` 前缀的尾部，其余丢弃
                        let keep = tag_prefix_tail(&self.buffer);
                        let cut = keep.unwrap_or(self.buffer.len());
                        self.buffer.drain(..cut);
                        break;
                    }
                }
            } else {
                match self.buffer.find(THINK_OPEN) {
                    Some(pos) => {
                        out.push_str(&self.buffer[..pos]);
                        self.buffer.drain(..pos + THINK_OPEN.len());
                        self.in_think = true;
                    }
                    None => {
                        // 正常文本：只保留可能是 `<think>` 前缀的尾部，其余输出
                        let keep = tag_prefix_tail(&self.buffer);
                        let cut = keep.unwrap_or(self.buffer.len());
                        out.push_str(&self.buffer[..cut]);
                        self.buffer.drain(..cut);
                        break;
                    }
                }
            }
        }
        out
    }

    /// 生成结束：返回可见残余（思考块未闭合则丢弃）。
    fn finish(&mut self) -> String {
        if self.in_think {
            self.buffer.clear();
            String::new()
        } else {
            std::mem::take(&mut self.buffer)
        }
    }
}

/// 若 `buffer` 末尾是 `<think>` / `</think>` 的前缀（跨 token 残片），返回其起始
/// 字节位置（需保留等待补全）；否则返回 `None`（整段可安全输出/丢弃）。
///
/// 只检查标签前缀，因此正常文本**不会**被尾部延迟截断。
fn tag_prefix_tail(buffer: &str) -> Option<usize> {
    const MAX: usize = THINK_CLOSE.len() - 1; // 最长标签 - 1
    let candidates = [THINK_OPEN, THINK_CLOSE];
    // 从后往前枚举 char 边界，检查后缀是否为标签前缀
    for (i, _) in buffer.char_indices().rev() {
        let suffix = &buffer[i..];
        if suffix.len() > MAX {
            break;
        }
        if candidates.iter().any(|tag| tag.starts_with(suffix)) {
            return Some(i);
        }
    }
    None
}

/// 构造传给 LLM 的输入：System prompt + 历史消息（多轮上下文）。
fn build_llm_input(system_prompt: &str, history: &[InputItem]) -> Vec<InputItem> {
    let mut input = vec![InputItem::Message(ChatMessage::new(
        ChatRole::System,
        system_prompt.to_string(),
    ))];
    input.extend(history.iter().cloned());
    input
}

/// 裁剪历史到最近 `max` 条（丢弃最早的多余消息）。
fn truncate_history(history: &mut Vec<InputItem>, max: usize) {
    if history.len() > max {
        let drop = history.len() - max;
        history.drain(..drop);
    }
}

/// 计算一段 f32 mono 音频的 RMS（均方根）音量，用于「真正说话」门控与静音累计。
fn chunk_rms(chunk: &[f32]) -> f32 {
    if chunk.is_empty() {
        return 0.0;
    }
    let sum: f32 = chunk.iter().map(|s| s * s).sum();
    (sum / chunk.len() as f32).sqrt()
}

/// ASR 反应：收集部分（流式字幕）与最终识别文本；一句说完（`is_final`）返回 `Stop`。
#[derive(Default)]
struct AsrCollector {
    final_text: Option<String>,
    partial: String,
}

impl AsrReaction for AsrCollector {
    fn on_result(&mut self, result: &AsrResult) -> ReactionOutcome {
        if result.is_final && !result.text.trim().is_empty() {
            self.final_text = Some(result.text.clone());
            return ReactionOutcome::Stop;
        }
        // 部分结果（未到 endpoint）：更新流式字幕
        if !result.text.is_empty() {
            self.partial = result.text.clone();
        }
        ReactionOutcome::Continue
    }
}

/// KWS 反应（Armed 待唤醒）：命中唤醒词即停止检测并记录关键词 → 切换 ASR。
#[derive(Default)]
struct WakeReaction {
    keyword: Option<String>,
}

impl Reaction for WakeReaction {
    fn on_keyword(&mut self, result: &KwsResult) -> ReactionOutcome {
        self.keyword = Some(result.keyword.clone());
        ReactionOutcome::Stop
    }
}

/// KWS 反应（Thinking/Speaking 打断监听）：命中即置位打断标志（继续监听，不 Stop）。
struct BargeInReaction<'a> {
    flag: &'a AtomicBool,
}

impl Reaction for BargeInReaction<'_> {
    fn on_keyword(&mut self, _result: &KwsResult) -> ReactionOutcome {
        self.flag.store(true, Ordering::Relaxed);
        ReactionOutcome::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reply_accumulator_splits_and_joins() {
        let mut r = ReplyAccumulator::new();
        // 增量 token，无句号前不切出
        let (visible, sentences) = r.push_token("你好，");
        assert_eq!(visible, "你好，");
        assert!(sentences.is_empty());
        assert!(r.push_token("世界").1.is_empty());
        // 句号切出完整句
        let (_, sentences) = r.push_token("。这是第二句");
        assert_eq!(sentences, vec!["你好，世界。".to_string()]);
        // 完整回复文本拼接正确
        assert_eq!(r.take_text().as_deref(), Some("你好，世界。这是第二句"));
    }

    #[test]
    fn test_reply_accumulator_finish_flushes_tail() {
        let mut r = ReplyAccumulator::new();
        r.push_token("没有标点的尾巴");
        // finish 返回残余句
        assert_eq!(r.finish().as_deref(), Some("没有标点的尾巴"));
        // take_text 仍含完整文本
        assert_eq!(r.take_text().as_deref(), Some("没有标点的尾巴"));
    }

    #[test]
    fn test_reply_accumulator_empty() {
        let mut r = ReplyAccumulator::new();
        assert!(r.push_token("  ").1.is_empty());
        assert_eq!(r.finish(), None);
        assert_eq!(r.take_text(), None);
    }

    #[test]
    fn test_thinking_block_filtered_from_tts_and_history() {
        let mut r = ReplyAccumulator::new();
        // 思考块内容不进入可见文本（不切句、不入历史、不上屏）
        let (visible, sentences) = r.push_token("<think>用户问\n");
        assert_eq!(visible, "");
        assert!(sentences.is_empty());
        let (visible, sentences) = r.push_token("我来分析一下。</think>");
        assert_eq!(visible, "");
        assert!(sentences.is_empty());
        // 闭合后的可见内容正常
        let (visible, sentences) = r.push_token("好的，这是回答。");
        assert_eq!(visible, "好的，这是回答。");
        assert_eq!(sentences, vec!["好的，这是回答。".to_string()]);
        // 历史只含可见文本（思考内容不进历史）
        assert_eq!(r.take_text().as_deref(), Some("好的，这是回答。"));
    }

    #[test]
    fn test_thinking_tag_split_across_tokens() {
        let mut r = ReplyAccumulator::new();
        // `<think>` 被拆成多段 token
        assert_eq!(r.push_token("<th").0, "");
        assert_eq!(r.push_token("ink>思考内容").0, "");
        assert_eq!(r.push_token("</th").0, "");
        assert_eq!(r.push_token("ink>答案").0, "答案");
        assert_eq!(r.take_text().as_deref(), Some("答案"));
    }

    #[test]
    fn test_unclosed_thinking_dropped_at_finish() {
        let mut r = ReplyAccumulator::new();
        r.push_token("<think>未闭合的思考");
        // finish 丢弃思考块，无残余
        assert_eq!(r.finish(), None);
        assert_eq!(r.take_text(), None);
    }

    #[test]
    fn test_thinking_without_close_then_normal() {
        let mut r = ReplyAccumulator::new();
        r.push_token("<think>思考");
        let (visible, _) = r.push_token("</think>正式回复");
        assert_eq!(visible, "正式回复");
        let (_, sentences) = r.push_token("。第二句");
        assert_eq!(sentences, vec!["正式回复。".to_string()]);
        assert_eq!(r.take_text().as_deref(), Some("正式回复。第二句"));
    }

    #[test]
    fn test_thinking_filter_multibyte_safety() {
        // 思考块内夹杂多字节中文，切分不得 panic（char 边界安全）
        let mut f = ThinkingFilter::default();
        let out = f.feed("<think>中文思考内容很长很长</think>可见");
        assert_eq!(out, "可见");
    }

    #[test]
    fn test_build_llm_input_prepends_system() {
        let history = vec![
            InputItem::Message(ChatMessage::new(ChatRole::User, "你好")),
            InputItem::Message(ChatMessage::new(ChatRole::Assistant, "你好！")),
        ];
        let input = build_llm_input("你是助手", &history);
        assert_eq!(input.len(), 3);
        assert!(matches!(
            &input[0],
            InputItem::Message(m) if m.role == ChatRole::System && m.content == "你是助手"
        ));
        assert!(matches!(
            &input[1],
            InputItem::Message(m) if m.role == ChatRole::User && m.content == "你好"
        ));
        assert!(matches!(
            &input[2],
            InputItem::Message(m) if m.role == ChatRole::Assistant && m.content == "你好！"
        ));
    }

    #[test]
    fn test_build_llm_input_empty_history() {
        let input = build_llm_input("你是助手", &[]);
        assert_eq!(input.len(), 1);
        assert!(matches!(&input[0], InputItem::Message(m) if m.role == ChatRole::System));
    }

    #[test]
    fn test_truncate_history_keeps_recent() {
        let mut history = vec![
            InputItem::Message(ChatMessage::new(ChatRole::User, "1")),
            InputItem::Message(ChatMessage::new(ChatRole::User, "2")),
            InputItem::Message(ChatMessage::new(ChatRole::User, "3")),
            InputItem::Message(ChatMessage::new(ChatRole::User, "4")),
        ];
        truncate_history(&mut history, 2);
        assert_eq!(history.len(), 2);
        // 保留最近的 3、4
        assert!(matches!(&history[0], InputItem::Message(m) if m.content == "3"));
        assert!(matches!(&history[1], InputItem::Message(m) if m.content == "4"));
    }

    #[test]
    fn test_truncate_history_within_limit_unchanged() {
        let mut history = vec![
            InputItem::Message(ChatMessage::new(ChatRole::User, "1")),
            InputItem::Message(ChatMessage::new(ChatRole::User, "2")),
        ];
        truncate_history(&mut history, 4);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_chunk_rms_values() {
        assert_eq!(chunk_rms(&[]), 0.0);
        assert_eq!(chunk_rms(&[0.0, 0.0, 0.0]), 0.0);
        // 恒定振幅 → RMS = 该振幅
        assert!((chunk_rms(&[0.5, 0.5]) - 0.5).abs() < 1e-6);
        // 峰值 1.0 正弦 → RMS ≈ 0.707
        let sine: Vec<f32> = (0..3200)
            .map(|i| ((i as f32 / 3200.0) * std::f32::consts::TAU).sin())
            .collect();
        assert!((chunk_rms(&sine) - 0.7071).abs() < 0.01);
        // 幅度更大 → RMS 更大（用于阈值门控判断）
        assert!(chunk_rms(&[0.9, 0.9]) > chunk_rms(&[0.1, 0.1]));
    }
}
