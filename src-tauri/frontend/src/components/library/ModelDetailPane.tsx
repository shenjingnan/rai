import { AudioWaveform, Brain, Loader2, type LucideIcon, Mic, Volume2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import type { ModelDetailState } from "@/hooks/useModelDetail";
import type { ModelDownloadsState } from "@/hooks/useModelDownloads";
import type { ModelLibraryState } from "@/hooks/useModelLibrary";
import { isInstallable } from "@/lib/catalog/query";
import { api } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import type { ModelArtifact, ModelCategory, UnifiedModelItem } from "@/types/catalog";
import { CompatibilityBadge } from "./CompatibilityBadge";
import { type ArtifactLocalState, ModelDownloadSection } from "./ModelDownloadSection";
import { ModelFilesTab } from "./ModelFilesTab";
import { ModelOverviewTab } from "./ModelOverviewTab";
import { ModelVariantsTab } from "./ModelVariantsTab";

const CAT_ICON: Record<ModelCategory, LucideIcon> = {
  llm: Brain,
  asr: Mic,
  tts: Volume2,
  kws: AudioWaveform,
};

type Tab = "overview" | "variants" | "files";

interface ModelDetailPaneProps {
  detail: ModelDetailState;
  lib: ModelLibraryState;
  downloads: ModelDownloadsState;
}

function titleOf(item: UnifiedModelItem): string {
  if (item.remote) return item.remote.displayName || item.modelId;
  return item.builtin?.displayName || item.modelId;
}

function fmt(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

/** 每个 artifact 的本地状态（已装/下载中）。 */
function artifactLocal(
  item: UnifiedModelItem,
  artifact: ModelArtifact,
  lib: ModelLibraryState,
  downloads: ModelDownloadsState,
): ArtifactLocalState {
  const installed =
    (lib.models ?? []).find((m) => {
      if (m.repoId === item.modelId) {
        return artifact.variant ? m.quantization === artifact.variant : true;
      }
      return false;
    }) ?? null;
  const task =
    [...downloads.tasks.values()].find(
      (t) => t.modelId === item.modelId && t.artifactId === artifact.id,
    ) ?? null;
  return { installed, task };
}

/** variant → 本地提示（版本 Tab 徽标）。 */
function buildLocalHints(
  item: UnifiedModelItem,
  artifacts: ModelArtifact[],
  lib: ModelLibraryState,
  downloads: ModelDownloadsState,
): Map<string, { installed: boolean; current: boolean; downloading: boolean }> {
  const map = new Map<string, { installed: boolean; current: boolean; downloading: boolean }>();
  for (const a of artifacts) {
    const local = artifactLocal(item, a, lib, downloads);
    map.set(a.variant ?? a.id, {
      installed: !!local.installed,
      current: local.installed?.current ?? false,
      downloading:
        !!local.task && (local.task.state === "downloading" || local.task.state === "queued"),
    });
  }
  return map;
}

/** 右侧详情（45%）。provider=building：内置精选走 manifest 下载；provider=huggingface：HF 流程。 */
export function ModelDetailPane({ detail, lib, downloads }: ModelDetailPaneProps) {
  const item = detail.selected;
  const [tab, setTab] = useState<Tab>("overview");
  const [selectedArtifactId, setSelectedArtifactId] = useState<string | null>(null);

  // 选中模型后：自动确认兼容性（Possible → files → Compatible/Unsupported）
  // biome-ignore lint/correctness/useExhaustiveDependencies: canonicalKey 是选择变化触发器
  useEffect(() => {
    if (item) {
      setTab("overview");
      setSelectedArtifactId(null);
      if (item.provider === "huggingface") {
        detail.ensureCompatibility();
      }
    }
  }, [item?.canonicalKey]);

  // 默认选中推荐 / 第一个可安装 artifact
  useEffect(() => {
    const artifacts = detail.compatibility?.artifacts ?? [];
    if (artifacts.length === 0) return;
    const recommended = detail.compatibility?.recommendedVariant;
    const preferred =
      artifacts.find(
        (a) => recommended && a.variant?.toUpperCase() === recommended.toUpperCase(),
      ) ??
      artifacts.find((a) => a.installable) ??
      artifacts[0];
    setSelectedArtifactId((prev) => prev ?? preferred.id);
  }, [detail.compatibility]);

  // 切到「文件列表」tab 时懒加载文件（ensureFiles 幂等，已加载则直接返回）
  useEffect(() => {
    if (tab === "files") {
      detail.ensureFiles();
    }
  }, [tab, detail.ensureFiles]);

  const selectedArtifact = useMemo(
    () => detail.compatibility?.artifacts.find((a) => a.id === selectedArtifactId) ?? null,
    [detail.compatibility, selectedArtifactId],
  );

  if (!item) {
    return null;
  }

  const Icon = CAT_ICON[item.modelType ?? item.builtin?.modelType ?? "llm"];
  const isHf = item.provider === "huggingface";

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {/* 固定顶部：头部 */}
      <div className="shrink-0">
        <div className="flex items-start gap-3">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-app-background text-text-secondary">
            <Icon className="h-5 w-5" />
          </span>
          <div className="min-w-0 flex-1">
            <h3 className="text-base font-semibold leading-tight text-text-primary">
              {titleOf(item)}
            </h3>
            <p className="mt-0.5 truncate text-xs text-text-secondary">
              {item.modelId}
              {item.remote?.lastModified ? ` · 更新 ${item.remote.lastModified.slice(0, 10)}` : ""}
            </p>
          </div>
          <CompatibilityBadge level={item.compatibility} />
        </div>
        <p className="mt-2 text-sm text-text-secondary">
          {item.remote?.description ?? item.builtin?.description ?? ""}
        </p>
        <p className="mt-1 text-[11px] text-text-muted">
          {(item.remote?.parameterCount ?? item.builtin?.parameterCount)
            ? `${item.remote?.parameterCount ?? item.builtin?.parameterCount} 参数 · `
            : ""}
          {item.remote ? (
            <>
              ↓ {fmt(item.remote.downloads)} · ♥ {fmt(item.remote.likes)}
              {item.remote.libraryName ? ` · ${item.remote.libraryName}` : ""}
            </>
          ) : (
            <></>
          )}
        </p>
      </div>

      {isHf ? (
        <>
          {/* 固定顶部：Tab 栏 */}
          <div className="flex shrink-0 items-center gap-1 border-b border-divider">
            {(
              [
                ["overview", "概览"],
                ["variants", "版本选择"],
                ["files", "文件列表"],
              ] as [Tab, string][]
            ).map(([t, label]) => (
              <button
                key={t}
                type="button"
                onClick={() => setTab(t)}
                className={cn(
                  "border-b-2 px-3 py-1.5 text-sm font-medium transition-colors",
                  tab === t
                    ? "border-blue-500 text-text-primary"
                    : "border-transparent text-text-secondary hover:text-text-primary",
                )}
              >
                {label}
              </button>
            ))}
          </div>

          {/* 可滚动内容区（仅 Tab 内容滚动，头部/Tab/下载区固定） */}
          <div className="min-h-0 flex-1 overflow-y-auto">
            {tab === "overview" && <ModelOverviewTab item={item} detail={detail.detail} />}

            {tab === "variants" &&
              (detail.loadingFiles && !detail.compatibility ? (
                <div className="flex items-center gap-2 py-6 text-xs text-text-muted">
                  <Loader2 className="h-4 w-4 animate-spin" /> 正在检查模型文件…
                </div>
              ) : detail.compatibility ? (
                <ModelVariantsTab
                  artifacts={detail.compatibility.artifacts}
                  selectedId={selectedArtifactId}
                  onSelect={setSelectedArtifactId}
                  recommendedVariant={detail.compatibility.recommendedVariant}
                  localHints={buildLocalHints(item, detail.compatibility.artifacts, lib, downloads)}
                />
              ) : (
                <p className="py-6 text-center text-xs text-text-muted">
                  {detail.filesError ?? "未检出可识别文件"}
                </p>
              ))}

            {tab === "files" &&
              (detail.loadingFiles && !detail.files ? (
                <div className="flex items-center gap-2 py-6 text-xs text-text-muted">
                  <Loader2 className="h-4 w-4 animate-spin" /> 正在加载文件列表…
                </div>
              ) : (
                <ModelFilesTab files={detail.files ?? []} />
              ))}
          </div>

          {/* 固定底部：仅版本选择的下载区 */}
          {tab === "variants" && detail.compatibility && (
            <div className="shrink-0 space-y-2">
              <p className="text-xs text-text-muted">{detail.compatibility.reason}</p>
              {selectedArtifact && (
                <ModelDownloadSection
                  artifact={selectedArtifact}
                  canInstall={isInstallable(item.compatibility) && selectedArtifact.installable}
                  local={artifactLocal(item, selectedArtifact, lib, downloads)}
                  systemRamGb={
                    lib.resources?.totalMemory ? lib.resources.totalMemory / 1024 ** 3 : null
                  }
                  onDownload={(a) => {
                    void downloads.enqueue({
                      modelId: item.modelId,
                      artifactId: a.id,
                      variant: a.variant,
                      artifactSource: "huggingface",
                      repoId: item.modelId,
                      revision: item.remote?.sha ?? null,
                      files: a.files,
                      modelType: item.modelType ?? undefined,
                    });
                  }}
                  onCancel={(taskId) => void downloads.cancel(taskId)}
                  onSetCurrent={(installId) => void lib.setCurrent(installId)}
                  onOpenDir={(installId) => void api.openModelDirectory({ id: installId })}
                  onViewOnHf={() => void api.openExternal(`https://huggingface.co/${item.modelId}`)}
                />
              )}
            </div>
          )}
        </>
      ) : (
        <BuiltinActions item={item} lib={lib} />
      )}
    </div>
  );
}

/** 内置精选操作（manifest 下载 / 导入 GGUF / 已安装+设为当前）。 */
function BuiltinActions({ item, lib }: { item: UnifiedModelItem; lib: ModelLibraryState }) {
  const regId = item.modelId;
  const installed = (lib.models ?? []).find((m) => m.id === regId || m.repoId === regId) ?? null;
  const installId = installed?.installId ?? installed?.id ?? regId;

  if (installed) {
    return (
      <div className="flex items-center gap-2">
        <span className="inline-flex items-center gap-1.5 text-xs text-emerald-600">
          <span className="h-1.5 w-1.5 rounded-full bg-current" />
          已安装
        </span>
        {!installed.current && (
          <Button size="sm" onClick={() => void lib.setCurrent(installId)}>
            设为当前模型
          </Button>
        )}
        <Button
          variant="outline"
          size="sm"
          className="shadow-none"
          onClick={() => void api.openModelDirectory({ id: installId })}
        >
          打开目录
        </Button>
      </div>
    );
  }
  return (
    <div className="flex items-center gap-2">
      <Button size="sm" onClick={() => void lib.download(regId)}>
        下载
      </Button>
    </div>
  );
}
