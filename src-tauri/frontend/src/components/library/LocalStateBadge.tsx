import { cn } from "@/lib/utils";

interface LocalStateBadgeProps {
  installedCount: number;
  hasCurrent: boolean;
  downloadingCount: number;
}

/** 本地状态徽标（已安装 / 当前模型 / 下载中）。 */
export function LocalStateBadge({
  installedCount,
  hasCurrent,
  downloadingCount,
}: LocalStateBadgeProps) {
  if (hasCurrent) {
    return (
      <span className="inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border border-blue-200 bg-blue-50 px-1.5 py-px text-[10px] font-medium text-blue-600">
        <span className="h-1 w-1 rounded-full bg-current" />
        当前模型
      </span>
    );
  }
  if (downloadingCount > 0) {
    return (
      <span className="inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border border-amber-200 bg-amber-50 px-1.5 py-px text-[10px] font-medium text-amber-600">
        <span className={cn("h-1 w-1 rounded-full bg-current")} />
        下载中
      </span>
    );
  }
  if (installedCount > 0) {
    return (
      <span className="inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border border-emerald-200 bg-emerald-50 px-1.5 py-px text-[10px] font-medium text-emerald-600">
        <span className="h-1 w-1 rounded-full bg-current" />
        已安装
      </span>
    );
  }
  return null;
}
