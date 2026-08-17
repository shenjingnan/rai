import { Check, Download, FolderOpen, Loader2, MoreHorizontal, Settings2 } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import type {
  LibraryModel,
  ModelLibraryProgress,
  ModelType,
  RuntimeStatus,
} from "@/types/modelLibrary";
import { formatBytes, LANGUAGE_LABELS, MODEL_TYPE_SHORT, TYPE_META, tagLabel } from "./libraryMeta";

function runtimeStatusLabel(status: RuntimeStatus): string | null {
  switch (status) {
    case "active":
      return "使用中";
    case "switching":
      return "正在加载…";
    case "pending_restart":
      return "下次启动生效";
    case "load_failed":
      return "加载失败";
    case "inactive":
      return null;
  }
}

function configHref(modelType: ModelType): string {
  return `/models/${modelType}`;
}

interface LibraryCardProps {
  model: LibraryModel;
  selected: boolean;
  onToggleSelect: (id: string) => void;
  downloadingId: string | null;
  progress: ModelLibraryProgress | null;
  onDownload: (id: string) => void;
  onCancelDownload: () => void;
  onUse: (id: string) => void;
  onImport: (model: LibraryModel) => void;
  onOpenDir: (model: LibraryModel) => void;
  onDetail: (model: LibraryModel) => void;
  onDelete: (model: LibraryModel) => void;
  onRemove: (model: LibraryModel) => void;
}

/** 模型卡片：横向 List Card，含状态机主操作 + ⋯ 菜单。 */
export function LibraryCard({
  model,
  selected,
  onToggleSelect,
  downloadingId,
  progress,
  onDownload,
  onCancelDownload,
  onUse,
  onImport,
  onOpenDir,
  onDetail,
  onDelete,
  onRemove,
}: LibraryCardProps) {
  const navigate = useNavigate();
  const meta = TYPE_META[model.modelType];
  const Icon = meta.icon;
  const isDownloading = downloadingId === model.id;
  const installed = model.installState === "installed";
  const invalid = model.installState === "invalid";
  const external = model.ownership === "external";
  const notInstalled = model.installState === "not_installed";

  const runtimeLabel = model.current ? runtimeStatusLabel(model.runtimeStatus) : null;

  // 下载/导入类次级操作：浅蓝柔化，避免实心蓝在列表中过重；「使用」保留为主操作。
  const softAction = "bg-primary/10 text-primary hover:bg-primary/15";

  let action: React.ReactNode = null;
  if (isDownloading) {
    action = (
      <Button variant="outline" size="sm" onClick={onCancelDownload} disabled={!progress}>
        <Loader2 className="h-4 w-4 animate-spin" />
        取消
      </Button>
    );
  } else if (invalid) {
    if (external && model.source === "local") {
      action = (
        <Button variant="outline" size="sm" onClick={() => onRemove(model)}>
          移除
        </Button>
      );
    } else if (external) {
      action = (
        <Button size="sm" className={softAction} onClick={() => onImport(model)}>
          <Download className="h-4 w-4" />
          重新导入
        </Button>
      );
    } else {
      action = (
        <Button size="sm" className={softAction} onClick={() => onDownload(model.id)}>
          <Download className="h-4 w-4" />
          重新安装
        </Button>
      );
    }
  } else if (notInstalled) {
    if (model.downloadable) {
      action = (
        <Button size="sm" className={softAction} onClick={() => onDownload(model.id)}>
          <Download className="h-4 w-4" />
          下载
        </Button>
      );
    } else {
      action = (
        <Button size="sm" className={softAction} onClick={() => onImport(model)}>
          导入 GGUF
        </Button>
      );
    }
  } else if (installed && model.current) {
    action = (
      <span className="inline-flex items-center gap-1.5 rounded-md border border-emerald-200 bg-emerald-50 px-3 py-1.5 text-sm font-medium text-emerald-600">
        <span className="h-1.5 w-1.5 rounded-full bg-current" />
        当前模型
      </span>
    );
  } else if (installed) {
    action = (
      <Button size="sm" onClick={() => onUse(model.id)}>
        使用
      </Button>
    );
  }

  return (
    <div
      className={cn(
        "rounded-[16px] border bg-panel-background px-4 py-3.5 transition-colors",
        model.current ? "border-emerald-200" : "border-panel-border",
      )}
    >
      <div className="flex items-start gap-3">
        {/* 批量选择 */}
        <button
          type="button"
          aria-label={selected ? "取消选择" : "选择模型"}
          onClick={() => onToggleSelect(model.id)}
          className={cn(
            "mt-1 flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors",
            selected
              ? "border-primary bg-primary text-primary-foreground"
              : "border-panel-border bg-white hover:border-primary",
          )}
        >
          {selected && <Check className="h-3 w-3" />}
        </button>

        <span
          className={cn(
            "flex h-9 w-9 shrink-0 items-center justify-center rounded-full",
            meta.iconClass,
          )}
        >
          <Icon className="h-4 w-4" />
        </span>

        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-sm font-semibold text-text-primary">{model.displayName}</p>
            <span
              className={cn(
                "rounded-full border px-1.5 py-px text-[10px] font-medium",
                meta.badgeClass,
              )}
            >
              {MODEL_TYPE_SHORT[model.modelType]}
            </span>
            <span className="text-xs text-text-secondary">
              {model.runtime} · {model.format}
            </span>
          </div>
          <p className="mt-0.5 truncate text-xs text-text-secondary">{model.description}</p>
          <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
            {model.languages.map((l) => (
              <span
                key={l}
                className="rounded bg-app-background px-1.5 py-px text-[11px] text-text-secondary"
              >
                {LANGUAGE_LABELS[l] ?? l}
              </span>
            ))}
            {model.tags.slice(0, 5).map((t) => (
              <span
                key={t}
                className="rounded bg-app-background px-1.5 py-px text-[11px] text-text-secondary"
              >
                {tagLabel(t)}
              </span>
            ))}
          </div>
          <p className="mt-1.5 text-[11px] text-text-muted">
            {model.parameterCount ? `${model.parameterCount} 参数 | ` : ""}
            {model.quantization ? `${model.quantization} | ` : ""}
            {model.sizeBytes ? `${formatBytes(model.sizeBytes)}` : ""}
            {installed && model.localPath ? ` | ${model.localPath}` : ""}
          </p>

          {/* 下载进度 */}
          {isDownloading && progress && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center justify-between text-xs">
                <span className="text-text-secondary">
                  {progress.stage === "preparing"
                    ? "准备下载…"
                    : `${Math.max(0, Math.round(progress.overallPercent))}%`}
                </span>
                {progress.totalBytes > 0 && (
                  <span className="text-text-muted">
                    {formatBytes(progress.bytesDownloaded)} / {formatBytes(progress.totalBytes)}
                  </span>
                )}
              </div>
              <Progress
                value={progress.stage === "preparing" ? 0 : Math.max(0, progress.overallPercent)}
              />
              <p className="truncate text-[11px] text-text-muted">{progress.message}</p>
            </div>
          )}
        </div>

        {/* 状态 + 主操作 */}
        <div className="flex shrink-0 items-center gap-2">
          {installed && !model.current && (
            <span className="flex items-center gap-1.5 whitespace-nowrap text-xs text-emerald-600">
              <span className="h-1.5 w-1.5 rounded-full bg-current" />
              已安装
            </span>
          )}
          {notInstalled && model.downloadable && (
            <span className="flex items-center gap-1.5 whitespace-nowrap text-xs text-text-muted">
              未安装
            </span>
          )}
          {notInstalled && !model.downloadable && (
            <span className="flex items-center gap-1.5 whitespace-nowrap text-xs text-text-muted">
              需本地导入
            </span>
          )}
          {invalid && (
            <span className="flex items-center gap-1.5 whitespace-nowrap text-xs text-red-600">
              <span className="h-1.5 w-1.5 rounded-full bg-current" />
              {external ? "文件已丢失" : "模型不完整"}
            </span>
          )}
          {model.current && runtimeLabel && (
            <span className="whitespace-nowrap text-xs text-text-secondary">{runtimeLabel}</span>
          )}
          {action}

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 shrink-0"
                aria-label="模型菜单"
              >
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {(installed || invalid) && (
                <>
                  <DropdownMenuItem onSelect={() => navigate(configHref(model.modelType))}>
                    <Settings2 className="h-4 w-4" />
                    打开模型配置
                  </DropdownMenuItem>
                  <DropdownMenuItem onSelect={() => model.localPath && onOpenDir(model)}>
                    <FolderOpen className="h-4 w-4" />
                    打开模型目录
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                </>
              )}
              <DropdownMenuItem onSelect={() => onDetail(model)}>查看详情</DropdownMenuItem>
              {(installed || (invalid && !external)) && (
                <>
                  <DropdownMenuSeparator />
                  {external ? (
                    <DropdownMenuItem
                      className="text-red-600 focus:text-red-600"
                      onSelect={() => onRemove(model)}
                    >
                      从模型库移除
                    </DropdownMenuItem>
                  ) : (
                    <DropdownMenuItem
                      className="text-red-600 focus:text-red-600"
                      onSelect={() => onDelete(model)}
                    >
                      卸载模型
                    </DropdownMenuItem>
                  )}
                </>
              )}
              {invalid && external && (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem
                    className="text-red-600 focus:text-red-600"
                    onSelect={() => onRemove(model)}
                  >
                    从模型库移除
                  </DropdownMenuItem>
                </>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </div>
  );
}
