import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  onVoiceSessionError,
  onVoiceSessionPlay,
  onVoiceSessionReply,
  onVoiceSessionReplyFinished,
  onVoiceSessionState,
  onVoiceSessionStopped,
  onVoiceSessionToken,
  onVoiceSessionTranscript,
  onVoiceSessionWake,
} from "@/lib/tauri";
import type { VoiceSessionPhase } from "@/types/tauri";

export interface TranscriptSegment {
  id: number;
  text: string;
  at: string;
}

/** 语音会话运行态（订阅后端 `voice-session-*` 事件）。 */
export interface VoiceSessionState {
  running: boolean;
  phase: VoiceSessionPhase;
  /** ASR 实时字幕（部分结果） */
  partial: string;
  /** 已完成的用户话语（最新在前） */
  userSegments: TranscriptSegment[];
  /** LLM 流式回复文本 */
  replyText: string;
  replyDone: boolean;
  /** 已入队合成的句子 */
  queuedSentences: string[];
  /** 正在播报的句子 */
  currentSentence: string | null;
  error: string | null;
  /** start/stop 在途标志 */
  pending: boolean;
  start: () => Promise<void>;
  stop: () => Promise<void>;
}

/**
 * 语音会话状态管理：初始化回读后端运行态，订阅 `voice-session-*` 事件；
 * start/stop 包装对应 command。桌宠窗口（无 RuntimeContext）与设置窗口共用。
 */
export function useVoiceSession(): VoiceSessionState {
  const [running, setRunning] = useState(false);
  const [phase, setPhase] = useState<VoiceSessionPhase>("idle");
  const [partial, setPartial] = useState("");
  const [userSegments, setUserSegments] = useState<TranscriptSegment[]>([]);
  const [replyText, setReplyText] = useState("");
  const [replyDone, setReplyDone] = useState(false);
  const [queuedSentences, setQueuedSentences] = useState<string[]>([]);
  const [currentSentence, setCurrentSentence] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const idRef = useRef(0);

  useEffect(() => {
    // 启动时回读后端状态（应用可能在 setup 已自动进入待唤醒）
    api
      .isVoiceSessionRunning()
      .then(setRunning)
      .catch(() => {});

    const unsubs = [
      onVoiceSessionState((p) => {
        setRunning(p.running);
        setPhase(p.state);
      }),
      onVoiceSessionWake(() => {}),
      onVoiceSessionTranscript((p) => {
        if (p.is_final) {
          const id = ++idRef.current;
          setUserSegments((prev) =>
            [{ id, text: p.text, at: new Date().toLocaleTimeString() }, ...prev].slice(0, 50),
          );
          setPartial("");
        } else {
          setPartial(p.text);
        }
      }),
      onVoiceSessionToken((p) => setReplyText((prev) => prev + p.delta)),
      onVoiceSessionReply((p) => setQueuedSentences((prev) => [...prev, p.sentence])),
      onVoiceSessionPlay((p) => setCurrentSentence(p.sentence)),
      onVoiceSessionReplyFinished(() => setReplyDone(true)),
      onVoiceSessionError((p) => setError(p.message)),
      onVoiceSessionStopped((p) => {
        setRunning(false);
        setPhase("idle");
        if (p.error) setError(p.error);
      }),
    ];

    return () => {
      unsubs.forEach((p) => p.then((fn) => fn()));
    };
  }, []);

  const start = useCallback(async () => {
    setPending(true);
    setError(null);
    // 新一轮开始前清空上一轮回复展示（LLM 加载完成后后端会发 state 事件）
    setReplyText("");
    setReplyDone(false);
    setQueuedSentences([]);
    setCurrentSentence(null);
    try {
      await api.startVoiceSession();
      setRunning(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }, []);

  const stop = useCallback(async () => {
    setPending(true);
    try {
      await api.stopVoiceSession();
      setRunning(false);
      setPhase("idle");
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }, []);

  return {
    running,
    phase,
    partial,
    userSegments,
    replyText,
    replyDone,
    queuedSentences,
    currentSentence,
    error,
    pending,
    start,
    stop,
  };
}
