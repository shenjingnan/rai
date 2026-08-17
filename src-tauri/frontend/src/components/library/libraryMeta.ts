import { AudioWaveform, Brain, type LucideIcon, Mic, Volume2 } from "lucide-react";
import type { ModelType } from "@/types/modelLibrary";

export interface TypeMeta {
  label: string;
  icon: LucideIcon;
  /** 圆形图标底（延续 ModelSummary 的能力色） */
  iconClass: string;
  /** Badge 浅色底 */
  badgeClass: string;
}

export const TYPE_META: Record<ModelType, TypeMeta> = {
  kws: {
    label: "唤醒词（KWS）",
    icon: AudioWaveform,
    iconClass: "bg-violet-100 text-violet-600",
    badgeClass: "border-violet-200 bg-violet-50 text-violet-600",
  },
  asr: {
    label: "语音识别（ASR）",
    icon: Mic,
    iconClass: "bg-blue-100 text-blue-600",
    badgeClass: "border-blue-200 bg-blue-50 text-blue-600",
  },
  llm: {
    label: "AI 大脑（LLM）",
    icon: Brain,
    iconClass: "bg-emerald-100 text-emerald-600",
    badgeClass: "border-emerald-200 bg-emerald-50 text-emerald-600",
  },
  tts: {
    label: "语音合成（TTS）",
    icon: Volume2,
    iconClass: "bg-amber-100 text-amber-600",
    badgeClass: "border-amber-200 bg-amber-50 text-amber-600",
  },
};

export const MODEL_TYPE_SHORT: Record<ModelType, string> = {
  kws: "KWS",
  asr: "ASR",
  llm: "LLM",
  tts: "TTS",
};

/** 稳定 tag id → 中文显示（业务逻辑不依赖中文字符串）。 */
export const TAG_LABELS: Record<string, string> = {
  "wake-word": "唤醒词",
  streaming: "实时流式",
  "voice-clone": "音色克隆",
  "high-accuracy": "高精度",
  lightweight: "轻量级",
  thinking: "思考模式",
  multilingual: "多语言",
  qwen3: "Qwen3",
  chat: "对话",
  instruct: "指令",
  bilingual: "双语",
  zipformer: "Zipformer",
};

export function tagLabel(tag: string): string {
  return TAG_LABELS[tag] ?? tag;
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export const LANGUAGE_LABELS: Record<string, string> = {
  zh: "中文",
  en: "English",
  multilingual: "多语言",
};
