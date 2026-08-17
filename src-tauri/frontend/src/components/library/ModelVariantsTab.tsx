import { Check, Sparkles } from "lucide-react";
import { formatBytes } from "@/components/library/libraryMeta";
import { estimateRamGb } from "@/lib/catalog/quantization";
import { cn } from "@/lib/utils";
import type { ModelArtifact } from "@/types/catalog";

interface ModelVariantsTabProps {
  artifacts: ModelArtifact[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  recommendedVariant: string | null;
  /** variant → 本地状态（已装 / 当前 / 下载中）。 */
  localHints: Map<string, { installed: boolean; current: boolean; downloading: boolean }>;
}

/** 版本选择（GGUF variant / sherpa 文件组）。 */
export function ModelVariantsTab({
  artifacts,
  selectedId,
  onSelect,
  recommendedVariant,
  localHints,
}: ModelVariantsTabProps) {
  return (
    <div className="space-y-1.5">
      {artifacts.map((a) => {
        const hint = localHints.get(a.variant ?? a.id);
        const recommended =
          !!recommendedVariant && recommendedVariant.toUpperCase() === a.variant?.toUpperCase();
        return (
          <button
            key={a.id}
            type="button"
            onClick={() => onSelect(a.id)}
            className={cn(
              "flex w-full items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-colors",
              selectedId === a.id
                ? "border-blue-300 bg-blue-50/50 ring-1 ring-blue-200"
                : "border-panel-border bg-panel-background hover:border-blue-200",
            )}
          >
            <span
              className={cn(
                "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border",
                selectedId === a.id ? "border-blue-500" : "border-text-muted",
              )}
            >
              {selectedId === a.id && <Check className="h-3 w-3 text-blue-600" />}
            </span>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-text-primary">{a.variant ?? a.name}</span>
                {recommended && (
                  <span className="inline-flex items-center gap-1 rounded-full bg-blue-50 px-1.5 py-px text-[10px] font-medium text-blue-600">
                    <Sparkles className="h-2.5 w-2.5" />
                    推荐
                  </span>
                )}
              </div>
              <p className="truncate text-[11px] text-text-muted">
                {a.files.length > 1 ? `${a.files.length} 个文件` : a.files[0]?.path}
                {a.totalSize != null ? ` · ${formatBytes(a.totalSize)}` : ""}
                {a.runtime === "llama.cpp" && a.totalSize != null
                  ? ` · 约需 ≥${estimateRamGb(a.totalSize)} GB 内存`
                  : ""}
              </p>
            </div>
            {hint?.current && (
              <span className="shrink-0 text-[10px] font-medium text-blue-600">当前模型</span>
            )}
            {!hint?.current && hint?.installed && (
              <span className="shrink-0 text-[10px] font-medium text-emerald-600">已安装</span>
            )}
            {hint?.downloading && (
              <span className="shrink-0 text-[10px] font-medium text-amber-600">下载中</span>
            )}
            {!a.installable && (
              <span className="shrink-0 text-[10px] font-medium text-red-500">不完整</span>
            )}
          </button>
        );
      })}
      {artifacts.length === 0 && (
        <p className="py-6 text-center text-xs text-text-muted">没有可识别的版本文件</p>
      )}
    </div>
  );
}
