import { Cpu, Gauge, HardDrive, MemoryStick, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import type { SystemResources } from "@/types/modelLibrary";
import { formatBytes } from "./libraryMeta";

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
  // 内存进度条与数值统一表示「已使用 / 总容量」（used = total - available）。
  const memUsedPct = resources
    ? resources.totalMemory > 0
      ? Math.max(0, resources.totalMemory - resources.availableMemory) / resources.totalMemory
      : 0
    : 0;
  // 磁盘进度条与数值统一表示「已使用 / 总容量」（used = total - available）。
  const diskUsedPct = resources
    ? resources.diskTotal > 0
      ? Math.max(0, resources.diskTotal - resources.diskAvailable) / resources.diskTotal
      : 0
    : 0;

  return (
    <section>
      <div className="flex items-center justify-between px-1">
        <h3 className="text-xs font-medium text-text-muted">系统资源</h3>
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 text-text-muted hover:text-text-secondary"
          onClick={onRefresh}
          disabled={loading}
          aria-label="刷新系统资源"
        >
          <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
        </Button>
      </div>
      <div className="mt-2 space-y-3 rounded-[14px] border border-panel-border bg-panel-background px-3.5 py-3">
        <div className="space-y-1">
          <div className="flex items-center justify-between text-xs">
            <span className="flex items-center gap-1.5 text-text-secondary">
              <MemoryStick className="h-3.5 w-3.5" />
              内存
            </span>
            <span className="text-text-muted">
              {resources
                ? `${formatMemory(Math.max(0, resources.totalMemory - resources.availableMemory))} / ${formatMemory(resources.totalMemory)}`
                : "—"}
            </span>
          </div>
          <Progress value={memUsedPct * 100} className="h-1.5" />
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
          <Progress value={diskUsedPct * 100} className="h-1.5" />
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
      </div>
    </section>
  );
}
