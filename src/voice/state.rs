/// 语音会话状态机。
///
/// 纯函数 `transition` 描述状态迁移；编排器把跨线程信号（KWS 唤醒、ASR 最终文本、
/// 打断、LLM 事件、合成结果）先汇成 [`SessionEvent`] 再调用它，状态迁移集中在
/// one place，便于单测。
///
/// ```text
/// Idle --Start--> Armed --KeywordDetected--> Listening --UserUtteranceFinal--> Thinking
///  ▲              │                                                             │
///  │ Stop          └──ReplyFinished──┐                    FirstSentenceEnqueued │
///  │                                │                                           ▼
///  └──────────────Stop───────────────┼─────────────────────────────► Speaking ──┘
///   Armed <--ReplyFinished-- Speaking│                                       ▲
///   Armed <----BargeIn----- Thinking|Speaking ────────────────────────────────┘
/// ```
///
/// `Armed`（待唤醒）是 KWS 门控：只有命中唤醒词才进入 `Listening`（ASR），
/// 否则不消费用户话语——避免「不说话也一直在识别」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// 未运行（初始/停止）
    Idle,
    /// 待唤醒：KWS 监听唤醒词
    Armed,
    /// 聆听用户（ASR 识别）
    Listening,
    /// 模型思考（LLM 生成中）
    Thinking,
    /// 播报（TTS 句级播放中，可能仍在生成后续句）
    Speaking,
}

/// 触发状态迁移的会话事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    /// 会话开始（Idle → Armed）
    Start,
    /// 命中唤醒词（Armed → Listening）
    KeywordDetected,
    /// 一句话说完（Listening → Thinking）
    UserUtteranceFinal,
    /// 首个句子已入队合成（Thinking → Speaking）
    FirstSentenceEnqueued,
    /// 回复播完 / 无内容可播（Thinking|Speaking → Armed）
    ReplyFinished,
    /// 打断（Thinking|Speaking → Armed）
    BargeIn,
    /// 停止（任意 → Idle）
    Stop,
}

/// 状态迁移函数。非法迁移返回 Err（含两个状态，便于定位编排逻辑错误）。
pub fn transition(state: SessionState, ev: SessionEvent) -> Result<SessionState, String> {
    use SessionEvent::*;
    use SessionState::*;
    let next = match (state, ev) {
        (Idle, Start) => Armed,
        (Armed, KeywordDetected) => Listening,
        (Listening, UserUtteranceFinal) => Thinking,
        (Thinking, FirstSentenceEnqueued) => Speaking,
        // 播完 / 思考阶段未切出任何句子（空回复）→ 回到待唤醒
        (Speaking, ReplyFinished) => Armed,
        (Thinking, ReplyFinished) => Armed,
        // 打断 → 回到待唤醒
        (Thinking | Speaking, BargeIn) => Armed,
        // Stop 从任意状态（含 Idle）回到 Idle
        (_, Stop) => Idle,
        (s, ev) => {
            return Err(format!("非法状态迁移: {s:?} --{ev:?}--> ?"));
        }
    };
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_transitions() {
        use SessionEvent::*;
        use SessionState::*;
        assert_eq!(transition(Idle, Start).unwrap(), Armed);
        assert_eq!(transition(Armed, KeywordDetected).unwrap(), Listening);
        assert_eq!(transition(Listening, UserUtteranceFinal).unwrap(), Thinking);
        assert_eq!(
            transition(Thinking, FirstSentenceEnqueued).unwrap(),
            Speaking
        );
        assert_eq!(transition(Speaking, ReplyFinished).unwrap(), Armed);
        // 思考中未切出句子（空回复）→ 回到待唤醒
        assert_eq!(transition(Thinking, ReplyFinished).unwrap(), Armed);
        // 思考中/播报中打断 → 回到待唤醒
        assert_eq!(transition(Thinking, BargeIn).unwrap(), Armed);
        assert_eq!(transition(Speaking, BargeIn).unwrap(), Armed);
    }

    #[test]
    fn test_stop_from_any_state_goes_idle() {
        use SessionEvent::*;
        use SessionState::*;
        for s in [Idle, Armed, Listening, Thinking, Speaking] {
            assert_eq!(transition(s, Stop).unwrap(), Idle);
        }
    }

    #[test]
    fn test_invalid_transitions_error() {
        use SessionEvent::*;
        use SessionState::*;
        let invalid: &[(SessionState, SessionEvent)] = &[
            (Idle, KeywordDetected),
            (Idle, UserUtteranceFinal),
            (Idle, FirstSentenceEnqueued),
            (Idle, ReplyFinished),
            (Idle, BargeIn),
            (Armed, Start),
            (Armed, UserUtteranceFinal),
            (Armed, FirstSentenceEnqueued),
            (Armed, ReplyFinished),
            (Armed, BargeIn),
            (Listening, Start),
            (Listening, KeywordDetected),
            (Listening, FirstSentenceEnqueued),
            (Listening, ReplyFinished),
            (Listening, BargeIn),
            (Thinking, Start),
            (Thinking, KeywordDetected),
            (Thinking, UserUtteranceFinal),
            (Thinking, Start),
            (Speaking, Start),
            (Speaking, KeywordDetected),
            (Speaking, UserUtteranceFinal),
            (Speaking, FirstSentenceEnqueued),
        ];
        for (s, ev) in invalid {
            let err = transition(*s, *ev).unwrap_err();
            assert!(err.contains("非法状态迁移"), "err: {err}");
        }
    }

    #[test]
    fn test_transition_roundtrip() {
        use SessionEvent::*;
        use SessionState::*;
        // 一次完整对话轮次的状态序列
        let mut s = transition(Idle, Start).unwrap();
        assert_eq!(s, Armed);
        s = transition(s, KeywordDetected).unwrap();
        assert_eq!(s, Listening);
        s = transition(s, UserUtteranceFinal).unwrap();
        assert_eq!(s, Thinking);
        s = transition(s, FirstSentenceEnqueued).unwrap();
        assert_eq!(s, Speaking);
        s = transition(s, ReplyFinished).unwrap();
        assert_eq!(s, Armed);
    }
}
