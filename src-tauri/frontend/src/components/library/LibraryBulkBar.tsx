import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { LibraryModel } from "@/types/modelLibrary";
import { formatBytes } from "./libraryMeta";

interface LibraryBulkBarProps {
  /** 当前筛选结果 */
  filtered: LibraryModel[];
  selectedIds: Set<string>;
  onSelectAll: () => void;
  onClear: () => void;
  onBatchDelete: () => void;
}

/** 批量操作栏：仅支持 managed 已安装且非 current 的模型批量卸载。 */
export function LibraryBulkBar({
  filtered,
  selectedIds,
  onSelectAll,
  onClear,
  onBatchDelete,
}: LibraryBulkBarProps) {
  if (selectedIds.size === 0) return null;

  const selected = filtered.filter((m) => selectedIds.has(m.id));
  const totalSize = selected.reduce((sum, m) => sum + (m.sizeBytes ?? 0), 0);
  // 只有 managed 且已安装且非 current 才可批量卸载
  const eligible = selected.filter(
    (m) => m.ownership === "managed" && m.installState === "installed" && !m.current,
  );

  return (
    <div className="sticky bottom-0 z-10 -mx-3 -mb-3 border-t border-panel-border bg-panel-background/95 px-4 py-3 backdrop-blur">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-4 text-sm">
          <span className="text-text-primary">已选中 {selectedIds.size} 个模型</span>
          <span className="text-xs text-text-muted">总大小：{formatBytes(totalSize)}</span>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" onClick={onSelectAll}>
            全选当前筛选
          </Button>
          <Button variant="ghost" size="sm" onClick={onClear}>
            清空选择
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="shadow-none"
            disabled={eligible.length === 0}
            onClick={onBatchDelete}
          >
            <Trash2 className="h-4 w-4" />
            批量卸载（{eligible.length}）
          </Button>
        </div>
      </div>
      <p className="mt-1 text-[11px] text-text-muted">
        批量卸载仅支持 ZapMomo 下载的已安装模型；当前使用的模型与外部本地模型不参与。
      </p>
    </div>
  );
}
