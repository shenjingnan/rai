import { AudioWaveform, Brain, Mic, Volume2 } from "lucide-react";
import { formatBytes } from "@/components/library/libraryMeta";
import { cn } from "@/lib/utils";
import type { ModelCategory, UnifiedModelItem } from "@/types/catalog";
import { CompatibilityBadge } from "./CompatibilityBadge";
import { LocalStateBadge } from "./LocalStateBadge";

const CAT_ICON: Record<ModelCategory, typeof Brain> = {
  llm: Brain,
  asr: Mic,
  tts: Volume2,
  kws: AudioWaveform,
};

function displayTitle(item: UnifiedModelItem): string {
  if (item.remote) return item.remote.displayName || item.modelId;
  return item.builtin?.displayName || item.modelId;
}

function displayDesc(item: UnifiedModelItem): string {
  if (item.remote?.description) return item.remote.description;
  if (item.builtin?.description) return item.builtin.description;
  return "—";
}

function catOf(item: UnifiedModelItem): ModelCategory | null {
  if (item.modelType) return item.modelType;
  if (item.builtin) return item.builtin.modelType;
  const p = item.remote?.pipelineTag ?? "";
  if (p.startsWith("automatic-speech")) return "asr";
  if (p === "text-to-speech") return "tts";
  if (p === "text-generation") return "llm";
  return null;
}

/** 模型列表卡片（紧凑；兼容性 Badge 与本地状态 Badge 视觉独立）。 */
export function ModelListCard({
  item,
  selected,
  onSelect,
}: {
  item: UnifiedModelItem;
  selected: boolean;
  onSelect: (item: UnifiedModelItem) => void;
}) {
  const cat = catOf(item);
  const Icon = cat ? CAT_ICON[cat] : null;
  const remote = item.remote;
  // 参数量（有则显示，无则不显示——不伪造）
  const params = remote?.parameterCount || item.builtin?.parameterCount || null;
  const metaLine = [
    remote?.libraryName || item.builtin?.runtime,
    remote?.pipelineTag || item.builtin?.format,
    ...(remote?.languages || item.builtin?.languages || []),
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <button
      type="button"
      onClick={() => onSelect(item)}
      className={cn(
        "w-full px-3.5 py-3 text-left transition-colors",
        selected ? "bg-blue-50/70" : "hover:bg-nav-hover/60",
      )}
    >
      <div className="flex items-start gap-2.5">
        {Icon && (
          <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-app-background text-text-secondary">
            <Icon className="h-4 w-4" />
          </span>
        )}
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-[13px] font-semibold leading-tight text-text-primary">
              {displayTitle(item)}
            </span>
            <CompatibilityBadge level={item.compatibility} />
            <LocalStateBadge
              installedCount={item.localSummary.installedArtifactCount}
              hasCurrent={item.localSummary.hasCurrentArtifact}
              downloadingCount={item.localSummary.activeDownloadCount}
            />
          </div>
          <p className="mt-0.5 truncate text-xs text-text-secondary">{displayDesc(item)}</p>
          {metaLine && <p className="mt-1 truncate text-[11px] text-text-muted">{metaLine}</p>}
          {(remote || item.recommendedVariant || params) && (
            <p className="mt-1 flex items-center gap-3 text-[11px] text-text-muted">
              {params && <span>{params}</span>}
              {remote && (
                <span>
                  ↓ {formatCount(remote.downloads)} · ♥ {formatCount(remote.likes)}
                  {remote.lastModified ? ` · 更新 ${remote.lastModified.slice(0, 10)}` : ""}
                </span>
              )}
              {item.recommendedVariant && (
                <span className="font-medium text-text-secondary">{item.recommendedVariant}</span>
              )}
            </p>
          )}
        </div>
      </div>
    </button>
  );
}

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export { formatBytes };
