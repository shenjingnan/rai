import { X } from "lucide-react";
import type { ModelDetailState } from "@/hooks/useModelDetail";
import type { ModelDownloadsState } from "@/hooks/useModelDownloads";
import type { ModelLibraryState } from "@/hooks/useModelLibrary";
import { cn } from "@/lib/utils";
import { ModelDetailPane } from "./ModelDetailPane";

interface ModelDetailDrawerProps {
  detail: ModelDetailState;
  lib: ModelLibraryState;
  downloads: ModelDownloadsState;
  onClose: () => void;
}

/**
 * 详情抽屉（占位式，无遮罩）：划出后**占据右侧区域**，左侧列表缩短。
 *
 * - 首次从关闭→打开：宽度 + 内容从右滑入（一次动画）。
 * - 已打开时切换模型：`open` 恒为 true，宽度/位移不变 → **无动画**，直接替换内容，
 *   便于连续点击不同模型看不同详情。
 * - 关闭：宽度收起，左侧恢复全宽。
 */
export function ModelDetailDrawer({ detail, lib, downloads, onClose }: ModelDetailDrawerProps) {
  const open = detail.selected !== null;
  const WIDTH = "w-[min(460px,88vw)]";
  return (
    <div
      className={cn(
        "shrink-0 overflow-hidden transition-[width] duration-300 ease-out",
        open ? WIDTH : "w-0",
      )}
    >
      <div
        className={cn(
          "flex h-full flex-col rounded-l-2xl border border-panel-border bg-panel-background transition-transform duration-300 ease-out",
          WIDTH,
          open ? "translate-x-0" : "translate-x-full",
        )}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-divider px-4 py-3">
          <span className="text-sm font-medium text-text-secondary">模型详情</span>
          <button
            type="button"
            onClick={onClose}
            aria-label="关闭详情"
            className="rounded-lg p-1 text-text-muted transition-colors hover:bg-nav-hover hover:text-text-primary"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="min-h-0 flex-1 p-4">
          {detail.selected && <ModelDetailPane detail={detail} lib={lib} downloads={downloads} />}
        </div>
      </div>
    </div>
  );
}
