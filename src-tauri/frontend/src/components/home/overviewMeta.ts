import { AudioLines, AudioWaveform, Brain, type LucideIcon, Mic, Volume2 } from "lucide-react";
import type { LlmState } from "@/hooks/useLlm";
import type { TtsState } from "@/hooks/useTts";
import type { VoiceSessionState } from "@/hooks/useVoiceSession";
import type { RuntimeState } from "@/providers/RuntimeContext";

/** 状态语义色（与模型页 ModelSummary / 各能力 meta 的语义完全一致）。 */
export type OverviewTone = "good" | "idle" | "loading" | "error";

export const OVERVIEW_STATUS_COLOR: Record<OverviewTone, string> = {
  good: "text-emerald-600",
  idle: "text-text-muted",
  loading: "text-blue-600",
  error: "text-red-600",
};

/** AI 能力小卡数据（纯展示：Icon + 名称 + 缩写 + 状态）。 */
export interface CapabilityStatus {
  key: "kws" | "asr" | "llm" | "tts" | "voice";
  name: string;
  code: string;
  icon: LucideIcon;
  accent: string;
  label: string;
  tone: OverviewTone;
}

export interface OverviewInput {
  kws: RuntimeState["kws"];
  asr: RuntimeState["asr"];
  llm: LlmState;
  tts: TtsState;
  voice: VoiceSessionState;
}

/**
 * KWS 状态：错误 > 监听中 > 已就绪/未启用 > 未配置。
 * `enabled=true` 但未在监听是合法状态（启动自动监听失败会静默降级，
 * 见 lib.rs setup），此时能力已配置并开启，展示「已就绪」而非「未启用」。
 */
function kwsStatus(kws: RuntimeState["kws"]): { label: string; tone: OverviewTone } {
  if (kws.listening.error) return { label: "异常", tone: "error" };
  if (kws.listening.isListening) return { label: "监听中", tone: "good" };
  const cfg = kws.config.config;
  if (cfg?.models_present) {
    return cfg.enabled ? { label: "已就绪", tone: "good" } : { label: "未启用", tone: "idle" };
  }
  return { label: "未配置", tone: "idle" };
}

/** ASR 状态：错误 > 启动中 > 识别中 > 已就绪 > 未配置（会话型按需启动，无「未启用」态）。 */
function asrStatus(asr: RuntimeState["asr"]): { label: string; tone: OverviewTone } {
  if (asr.listening.error) return { label: "异常", tone: "error" };
  if (asr.listening.pending) return { label: "启动中", tone: "loading" };
  if (asr.listening.isListening) return { label: "识别中", tone: "good" };
  if (asr.config.config?.models_present) return { label: "已就绪", tone: "good" };
  return { label: "未配置", tone: "idle" };
}

/** LLM 状态：错误 > 生成中 > 加载中 > 运行中 > 未加载 > 未配置（词汇沿用 llmMeta）。 */
function llmStatus(llm: LlmState): { label: string; tone: OverviewTone } {
  if (llm.error) return { label: "异常", tone: "error" };
  if (llm.generating) return { label: "生成中", tone: "loading" };
  if (llm.loading) return { label: "加载中", tone: "loading" };
  if (llm.ready) return { label: "运行中", tone: "good" };
  if (llm.config?.models_present) return { label: "未加载", tone: "idle" };
  return { label: "未配置", tone: "idle" };
}

/** TTS 状态：配置错误 > 合成中 > 未配置 > 已关闭 > 已就绪（顺序沿用 ttsMeta：模型缺失优先于已关闭）。 */
function ttsStatus(tts: TtsState): { label: string; tone: OverviewTone } {
  if (tts.configError) return { label: "异常", tone: "error" };
  if (tts.synthesizing) return { label: "合成中", tone: "loading" };
  const cfg = tts.config;
  if (!cfg) return { label: "加载中", tone: "idle" };
  if (!cfg.models_present) return { label: "未配置", tone: "idle" };
  if (cfg.enabled === false) return { label: "已关闭", tone: "idle" };
  return { label: "已就绪", tone: "good" };
}

/** 语音会话状态：错误 > 启动中 > 欢迎中/待唤醒/聆听中/思考中/播报中 > 未启动。 */
function voiceStatus(voice: VoiceSessionState): { label: string; tone: OverviewTone } {
  if (voice.error) return { label: "异常", tone: "error" };
  if (voice.running && voice.phase === "idle") return { label: "启动中", tone: "loading" };
  switch (voice.phase) {
    case "armed":
      return { label: "待唤醒", tone: "good" };
    case "greeting":
      return { label: "欢迎中", tone: "loading" };
    case "waiting_speech":
    case "listening":
      return { label: "聆听中", tone: "good" };
    case "thinking":
      return { label: "思考中", tone: "loading" };
    case "speaking":
      return { label: "播报中", tone: "loading" };
    default:
      return { label: "未启动", tone: "idle" };
  }
}

/**
 * 概览页 AI 能力状态推导（纯函数）：基于真实 runtime 字段推导，
 * 不维护第二套状态源。顺序固定为 KWS / ASR / LLM / TTS（与模型摘要一致）。
 */
export function deriveOverview(input: OverviewInput): CapabilityStatus[] {
  const { kws, asr, llm, tts, voice } = input;
  const kwsState = kwsStatus(kws);
  const asrState = asrStatus(asr);
  const llmState = llmStatus(llm);
  const ttsState = ttsStatus(tts);
  const voiceState = voiceStatus(voice);

  return [
    {
      key: "kws",
      name: "唤醒词",
      code: "KWS",
      icon: AudioWaveform,
      accent: "bg-violet-100 text-violet-600",
      ...kwsState,
    },
    {
      key: "asr",
      name: "语音识别",
      code: "ASR",
      icon: Mic,
      accent: "bg-blue-100 text-blue-600",
      ...asrState,
    },
    {
      key: "llm",
      name: "AI 大脑",
      code: "LLM",
      icon: Brain,
      accent: "bg-emerald-100 text-emerald-600",
      ...llmState,
    },
    {
      key: "tts",
      name: "语音合成",
      code: "TTS",
      icon: Volume2,
      accent: "bg-amber-100 text-amber-600",
      ...ttsState,
    },
    {
      key: "voice",
      name: "语音会话",
      code: "VOICE",
      icon: AudioLines,
      accent: "bg-pink-100 text-pink-600",
      ...voiceState,
    },
  ];
}
