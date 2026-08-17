import {
  AudioWaveform,
  Brain,
  ChevronRight,
  Database,
  type LucideIcon,
  Mic,
  RefreshCw,
  Volume2,
} from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

type StatusTone = "good" | "idle" | "loading" | "error";

const STATUS_COLOR: Record<StatusTone, string> = {
  good: "text-emerald-600",
  idle: "text-text-muted",
  loading: "text-blue-600",
  error: "text-red-600",
};

interface SummaryRowData {
  accent: string;
  icon: LucideIcon;
  name: string;
  model: string;
  runtime: string;
  path: string | null;
  statusText: string;
  statusTone: StatusTone;
  gearHref?: string;
}

/** 模型摘要单行：整行可点击进入对应配置页；右侧状态 + chevron 指示。 */
function SummaryRow({ row }: { row: SummaryRowData }) {
  const Icon = row.icon;
  const content = (
    <>
      <span
        className={cn("flex h-9 w-9 shrink-0 items-center justify-center rounded-full", row.accent)}
      >
        <Icon className="h-4 w-4" />
      </span>

      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-text-primary">{row.name}</p>
        <p className="truncate text-xs text-text-secondary">{row.model}</p>
      </div>

      <div className="hidden min-w-0 flex-1 flex-col items-end gap-0.5 sm:flex">
        <p className="text-xs text-text-secondary">{row.runtime}</p>
        {row.path && (
          <p
            className="max-w-[240px] truncate font-mono text-[11px] text-text-muted"
            title={row.path}
          >
            {row.path}
          </p>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-3">
        <span
          className={cn(
            "flex items-center gap-1.5 whitespace-nowrap text-xs",
            STATUS_COLOR[row.statusTone],
          )}
        >
          <span className="h-1.5 w-1.5 rounded-full bg-current" />
          {row.statusText}
        </span>
        {row.gearHref && <ChevronRight className="h-4 w-4 shrink-0 text-text-muted" />}
      </div>
    </>
  );

  const rowClass = "flex items-center gap-4 px-5 py-3.5";

  if (!row.gearHref) {
    return <div className={rowClass}>{content}</div>;
  }

  return (
    <Link
      to={row.gearHref}
      aria-label={`配置${row.name}`}
      className={cn(rowClass, "transition-colors hover:bg-nav-hover")}
    >
      {content}
    </Link>
  );
}

/** 模型摘要：分组 List（macOS Settings 风格），非 DataTable。 */
export function ModelSummary() {
  const { kws, asr, llm, tts } = useRuntime();
  const [refreshing, setRefreshing] = useState(false);

  const refreshAll = async () => {
    setRefreshing(true);
    try {
      await Promise.all([
        kws.config.refresh(),
        asr.config.refresh(),
        llm.refreshConfig(),
        tts.refreshConfig(),
      ]);
    } finally {
      setRefreshing(false);
    }
  };

  const llmConfigured = llm.config?.models_present ?? false;
  const asrConfigured = asr.config?.config?.models_present ?? false;
  const kwsConfigured = kws.config?.config?.models_present ?? false;
  const ttsConfigured = tts.config?.models_present ?? false;
  const ttsEnabled = tts.config?.enabled ?? true;

  const rows: SummaryRowData[] = [
    {
      accent: "bg-violet-100 text-violet-600",
      icon: AudioWaveform,
      name: "唤醒词（KWS）",
      model: kwsConfigured ? basename(kws.config?.config?.model_dir ?? "") : "未配置模型",
      runtime: "sherpa-onnx",
      path: kwsConfigured ? (kws.config?.config?.model_dir ?? null) : null,
      statusText: kws.listening.error
        ? "错误"
        : kws.listening.isListening
          ? "监听中"
          : kwsConfigured
            ? "未启用"
            : "未配置模型",
      statusTone: kws.listening.error ? "error" : kws.listening.isListening ? "good" : "idle",
      gearHref: "/models/kws",
    },
    {
      accent: "bg-blue-100 text-blue-600",
      icon: Mic,
      name: "语音识别（ASR）",
      model: asrConfigured ? basename(asr.config?.config?.model_dir ?? "") : "未配置模型",
      runtime: "sherpa-onnx",
      path: asrConfigured ? (asr.config?.config?.model_dir ?? null) : null,
      statusText: asr.listening.error
        ? "错误"
        : asr.listening.isListening
          ? "识别中"
          : asrConfigured
            ? "未启用"
            : "未配置模型",
      statusTone: asr.listening.error ? "error" : asr.listening.isListening ? "good" : "idle",
      gearHref: "/models/asr",
    },
    {
      accent: "bg-emerald-100 text-emerald-600",
      icon: Brain,
      name: "AI 大脑（LLM）",
      model: llmConfigured ? basename(llm.config?.model_path ?? "") : "未配置模型",
      runtime: "llama.cpp",
      path: llmConfigured ? (llm.config?.model_path ?? null) : null,
      statusText: llm.error
        ? "错误"
        : llm.loading
          ? "加载中"
          : llm.ready
            ? "运行中"
            : llmConfigured
              ? "未启用"
              : "未配置模型",
      statusTone: llm.error ? "error" : llm.loading ? "loading" : llm.ready ? "good" : "idle",
      gearHref: "/models/llm",
    },
    {
      accent: "bg-amber-100 text-amber-600",
      icon: Volume2,
      name: "语音合成（TTS）",
      model: ttsConfigured ? basename(tts.config?.model_dir ?? "") : "未配置模型",
      runtime: "sherpa-onnx",
      path: ttsConfigured ? (tts.config?.model_dir ?? null) : null,
      statusText: !ttsEnabled
        ? "已关闭"
        : tts.synthesizing
          ? "合成中"
          : ttsConfigured
            ? "已就绪"
            : "未配置模型",
      statusTone: !ttsEnabled
        ? "idle"
        : tts.synthesizing
          ? "loading"
          : ttsConfigured
            ? "good"
            : "idle",
      gearHref: "/models/tts",
    },
  ];

  return (
    <section className="rounded-[16px] border border-panel-border bg-panel-background">
      <div className="flex flex-wrap items-center justify-between gap-2 px-5 py-4">
        <h2 className="text-base font-semibold text-text-primary">模型摘要</h2>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" onClick={refreshAll} disabled={refreshing}>
            <RefreshCw className={cn("h-4 w-4", refreshing && "animate-spin")} />
            刷新状态
          </Button>
          <Button variant="outline" size="sm" className="shadow-none" asChild>
            <Link to="/models/library">
              <Database className="h-4 w-4" />
              管理模型
            </Link>
          </Button>
        </div>
      </div>
      <div className="divide-y divide-[#eef1f6]">
        {rows.map((row) => (
          <SummaryRow key={row.name} row={row} />
        ))}
      </div>
    </section>
  );
}
