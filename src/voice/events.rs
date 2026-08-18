/// 语音会话事件与默认 CLI 输出。
///
/// `VoiceEvent` 是编排器的统一事件输出（替代 `VoiceSession` 内散落的
/// `println!/print!/eprintln!`）。宿主注入一个 `Box<dyn Fn(VoiceEvent) + Send>`：
/// CLI 用 [`cli_sink`]（逐字节复刻原有控制台输出，`zapmomo voice run` 行为不变），
/// Tauri 用 `app.emit` sink（转发为 `voice-session-*` 事件给前端）。
use crate::voice::state::SessionState;
use serde::Serialize;

/// 语音会话事件（`Serialize` 供 Tauri 跨进程转发给前端）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceEvent {
    /// 会话开始（`[会话] 开始...`）
    Started,
    /// 状态迁移（`[会话] 状态 -> {:?}`）
    State { state: SessionState },
    /// 唤醒词命中（`[唤醒] 检测到: {keyword}`）
    Wake { keyword: String },
    /// ASR 转写（部分/最终；最终对应 `[用户]`）
    Transcript { text: String, is_final: bool },
    /// LLM 流式可见文本增量（思考块已被过滤）
    Token { delta: String },
    /// 切句入队合成（`[合成] {sentence}`）
    ReplySentence { sentence: String },
    /// 合成结果开始播放（`[播放] {sentence}`）
    PlaySentence { sentence: String },
    /// 一轮回复生成结束（`[回复完成] {reason}`）
    ReplyFinished { reason: String },
    /// 错误（LLM / 合成 / 打断）
    Error { kind: ErrorKind, message: String },
    /// 唤醒词打断（`[打断] 检测到唤醒词...`）
    BargeIn,
    /// 会话停止（结束 / 达最大轮数）
    Stopped { reason: StoppedReason, turns: u32 },
}

/// 错误来源。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Llm,
    Synth,
    BargeIn,
}

/// 停止原因。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum StoppedReason {
    MaxTurns { max: u32 },
    Manual,
}

/// 默认 CLI sink：`zapmomo voice run` 的输出格式逐字节复刻原 `println!`。
pub fn cli_sink(ev: VoiceEvent) {
    use std::io::Write;
    match ev {
        VoiceEvent::Started => println!("[会话] 开始（Ctrl-C 退出）。喊唤醒词开始对话。"),
        VoiceEvent::State { state } => println!("[会话] 状态 -> {state:?}"),
        VoiceEvent::Wake { keyword } => println!("\n[唤醒] 检测到: {keyword}，开始聆听"),
        VoiceEvent::Transcript { text, is_final } => {
            if is_final {
                println!("\n[用户] {text}");
            } else if !text.is_empty() {
                print!("\r[识别] {text}");
                let _ = Write::flush(&mut std::io::stdout());
            }
        }
        VoiceEvent::Token { delta } => {
            print!("{delta}");
            let _ = Write::flush(&mut std::io::stdout());
        }
        VoiceEvent::ReplySentence { sentence } => println!("  [合成] {sentence}"),
        VoiceEvent::PlaySentence { sentence } => println!("  [播放] {sentence}"),
        VoiceEvent::ReplyFinished { reason } => {
            println!(); // 结束 token 流的一行
            println!("[回复完成] {reason}");
        }
        VoiceEvent::Error { kind, message } => match kind {
            ErrorKind::Llm => eprintln!("[LLM 错误] {message}"),
            ErrorKind::Synth => eprintln!("[合成错误] {message}"),
            ErrorKind::BargeIn => eprintln!("[打断] {message}"),
        },
        VoiceEvent::BargeIn => println!("\n[打断] 检测到唤醒词，回到待唤醒"),
        VoiceEvent::Stopped { reason, turns } => match reason {
            StoppedReason::MaxTurns { max } => println!("[会话] 已达最大轮数 {max}，退出"),
            StoppedReason::Manual => println!("[会话] 结束（共 {turns} 轮）"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::state::SessionState;

    #[test]
    fn test_voice_event_serialize_shape() {
        let cases = [
            (
                VoiceEvent::State {
                    state: SessionState::Armed,
                },
                r#"{"type":"state","state":"armed"}"#,
            ),
            (
                VoiceEvent::Transcript {
                    text: "今天天气".to_string(),
                    is_final: false,
                },
                r#"{"type":"transcript","text":"今天天气","is_final":false}"#,
            ),
            (
                VoiceEvent::ReplyFinished {
                    reason: "Eos".to_string(),
                },
                r#"{"type":"reply_finished","reason":"Eos"}"#,
            ),
            (
                VoiceEvent::Error {
                    kind: ErrorKind::Llm,
                    message: "加载失败".to_string(),
                },
                r#"{"type":"error","kind":"llm","message":"加载失败"}"#,
            ),
            (
                VoiceEvent::Stopped {
                    reason: StoppedReason::MaxTurns { max: 5 },
                    turns: 5,
                },
                r#"{"type":"stopped","reason":{"reason":"max_turns","max":5},"turns":5}"#,
            ),
        ];
        for (ev, expected) in cases {
            assert_eq!(serde_json::to_string(&ev).unwrap(), expected);
        }
    }

    #[test]
    fn test_cli_sink_all_variants_no_panic() {
        // 遍历所有变体调用 cli_sink，确保不 panic、不抛错
        let events = vec![
            VoiceEvent::Started,
            VoiceEvent::State {
                state: SessionState::Listening,
            },
            VoiceEvent::Wake {
                keyword: "你好小智".to_string(),
            },
            VoiceEvent::Transcript {
                text: "你".to_string(),
                is_final: false,
            },
            VoiceEvent::Transcript {
                text: "你好".to_string(),
                is_final: true,
            },
            VoiceEvent::Token {
                delta: "今天".to_string(),
            },
            VoiceEvent::ReplySentence {
                sentence: "今天天气不错。".to_string(),
            },
            VoiceEvent::PlaySentence {
                sentence: "今天天气不错。".to_string(),
            },
            VoiceEvent::ReplyFinished {
                reason: "Eos".to_string(),
            },
            VoiceEvent::Error {
                kind: ErrorKind::Llm,
                message: "x".to_string(),
            },
            VoiceEvent::Error {
                kind: ErrorKind::Synth,
                message: "x".to_string(),
            },
            VoiceEvent::Error {
                kind: ErrorKind::BargeIn,
                message: "x".to_string(),
            },
            VoiceEvent::BargeIn,
            VoiceEvent::Stopped {
                reason: StoppedReason::Manual,
                turns: 3,
            },
            VoiceEvent::Stopped {
                reason: StoppedReason::MaxTurns { max: 3 },
                turns: 3,
            },
        ];
        for ev in events {
            cli_sink(ev);
        }
    }
}
