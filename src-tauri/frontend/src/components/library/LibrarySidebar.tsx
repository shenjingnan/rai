import { Cpu, Gauge, HardDrive, MemoryStick, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import type { LibraryModel, SystemResources } from "@/types/modelLibrary";
import { formatBytes, tagLabel } from "./libraryMeta";

// -------------------------------------------------------------- 热门标签 ----

interface PopularTagsProps {
  models: LibraryModel[];
  activeTag: string | null;
  onToggleTag: (tag: string) => void;
}

/** 热门标签：从当前类型下模型聚合数量，点击组合过滤。 */
export function PopularTags({ models, activeTag, onToggleTag }: PopularTagsProps) {
  const counts = new Map<string, number>();
  for (const m of models) {
    for (const t of m.tags) {
      counts.set(t, (counts.get(t) ?? 0) + 1);
    }
  }
  const tags = [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12);

  if (tags.length === 0) return null;

  return (
    <section>
      <h3 className="px-1 text-xs font-medium text-text-muted">热门标签</h3>
      <ul className="mt-2 space-y-0.5">
        {tags.map(([tag, count]) => {
          const active = activeTag === tag;
          return (
            <li key={tag}>
              <button
                type="button"
                onClick={() => onToggleTag(tag)}
                className={cn(
                  "flex w-full items-center justify-between gap-2 rounded-lg px-2.5 py-1.5 text-left text-sm transition-colors",
                  active
                    ? "bg-nav-active text-primary"
                    : "text-text-secondary hover:bg-nav-hover hover:text-text-primary",
                )}
              >
                <span className="truncate">{tagLabel(tag)}</span>
                <span
                  className={cn(
                    "shrink-0 text-[11px] tabular-nums",
                    active ? "text-primary" : "text-text-muted",
                  )}
                >
                  {count}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

// ------------------------------------------------------------ 系统资源卡 ----

interface SystemResourcesCardProps {
  resources: SystemResources | null;
  loading: boolean;
  onRefresh: () => void;
}

function formatMemory(bytes: number): string {
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** 系统资源：内存 / 磁盘 / CPU（打开页面取一次，点「资源检测」再取一次，不做轮询）。 */
export function SystemResourcesCard({ resources, loading, onRefresh }: SystemResourcesCardProps) {
  const memPct = resources
    ? resources.totalMemory > 0
      ? resources.availableMemory / resources.totalMemory
      : 0
    : 0;
  const diskPct = resources
    ? resources.diskTotal > 0
      ? resources.diskAvailable / resources.diskTotal
      : 0
    : 0;

  return (
    <section>
      <h3 className="px-1 text-xs font-medium text-text-muted">系统资源</h3>
      <div className="mt-2 space-y-3 rounded-[14px] border border-panel-border bg-panel-background px-3.5 py-3">
        <div className="space-y-1">
          <div className="flex items-center justify-between text-xs">
            <span className="flex items-center gap-1.5 text-text-secondary">
              <MemoryStick className="h-3.5 w-3.5" />
              内存
            </span>
            <span className="text-text-muted">
              {resources
                ? `${formatMemory(resources.availableMemory)} / ${formatMemory(resources.totalMemory)}`
                : "—"}
            </span>
          </div>
          <Progress value={memPct * 100} className="h-1.5" />
        </div>

        <div className="space-y-1">
          <div className="flex items-center justify-between text-xs">
            <span className="flex items-center gap-1.5 text-text-secondary">
              <HardDrive className="h-3.5 w-3.5" />
              磁盘
            </span>
            <span className="text-text-muted">
              {resources ? `${formatBytes(resources.diskAvailable)} 可用` : "—"}
            </span>
          </div>
          <Progress value={diskPct * 100} className="h-1.5" />
        </div>

        <div className="flex items-center justify-between text-xs">
          <span className="flex items-center gap-1.5 text-text-secondary">
            <Cpu className="h-3.5 w-3.5" />
            CPU
          </span>
          <span className="flex items-center gap-1 text-text-muted">
            <Gauge className="h-3.5 w-3.5" />
            {resources ? `${resources.cpuUsage.toFixed(0)}%` : "—"}
          </span>
        </div>

        <Button
          variant="outline"
          size="sm"
          className="w-full shadow-none"
          onClick={onRefresh}
          disabled={loading}
        >
          <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
          资源检测
        </Button>
      </div>
    </section>
  );
}
