import { ExternalLink, Loader2 } from "lucide-react";
import { formatBytes } from "@/components/library/libraryMeta";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { estimateRamGb } from "@/lib/catalog/quantization";
import type { DownloadTaskView, ModelArtifact } from "@/types/catalog";
import type { LibraryModel } from "@/types/modelLibrary";

export interface ArtifactLocalState {
  installed: LibraryModel | null;
  task: DownloadTaskView | null;
}

interface ModelDownloadSectionProps {
  artifact: ModelArtifact;
  /** 是否允许一键安装（兼容性 Verified/Compatible 且 artifact.installable）。 */
  canInstall: boolean;
  local: ArtifactLocalState;
  /** 本机总内存（GB，可选；用于对比 LLM 估算需求）。 */
  systemRamGb?: number | null;
  onDownload: (artifact: ModelArtifact, immediate: boolean) => void;
  onCancel: (taskId: string) => void;
  onSetCurrent: (installId: string) => void;
  onOpenDir: (installId: string) => void;
  onViewOnHf: () => void;
}

/** 右下下载区：文件/大小 + 状态机操作（下载/取消/已安装/设为当前/暂不支持）。 */
export function ModelDownloadSection({
  artifact,
  canInstall,
  local,
  systemRamGb,
  onDownload,
  onCancel,
  onSetCurrent,
  onOpenDir,
  onViewOnHf,
}: ModelDownloadSectionProps) {
  const fileLabel =
    artifact.files.length > 1
      ? `${artifact.files.length} 个文件`
      : (artifact.files[0]?.path ?? artifact.name);
  const task = local.task;

  const renderAction = () => {
    const installed = local.installed;
    if (installed) {
      if (installed.current) {
        return (
          <span className="inline-flex items-center gap-1.5 text-xs text-blue-600">
            <span className="h-1.5 w-1.5 rounded-full bg-current" />
            当前模型
          </span>
        );
      }
      const installId = installed.installId ?? installed.id;
      return (
        <>
          <Button size="sm" onClick={() => installId && onSetCurrent(installId)}>
            设为当前模型
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="shadow-none"
            onClick={() => installId && onOpenDir(installId)}
          >
            打开目录
          </Button>
        </>
      );
    }
    if (task) {
      if (task.state === "downloading" || task.state === "queued") {
        return (
          <>
            <span className="text-xs text-text-secondary">{Math.round(task.progress)}%</span>
            <Button variant="outline" size="sm" onClick={() => onCancel(task.taskId)}>
              <Loader2 className="h-4 w-4 animate-spin" />
              取消
            </Button>
          </>
        );
      }
      return <span className="text-xs text-text-secondary">{task.state}</span>;
    }
    if (canInstall) {
      return (
        <>
          <Button size="sm" onClick={() => onDownload(artifact, false)}>
            加入下载队列
          </Button>
          <Button size="sm" onClick={() => onDownload(artifact, true)}>
            立即下载
          </Button>
        </>
      );
    }
    return (
      <Button variant="outline" size="sm" className="shadow-none" onClick={onViewOnHf}>
        <ExternalLink className="h-3.5 w-3.5" />在 Hugging Face 查看
      </Button>
    );
  };

  return (
    <div className="rounded-[14px] border border-panel-border bg-panel-background px-3.5 py-3">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-xs font-medium text-text-primary">{fileLabel}</p>
          <p className="text-[11px] text-text-muted">
            {artifact.totalSize != null ? formatBytes(artifact.totalSize) : "大小未知"}
            {artifact.runtime ? ` · ${artifact.runtime}` : ""}
            {artifact.runtime === "llama.cpp" && artifact.totalSize != null && (
              <>
                {" · "}约需 ≥{estimateRamGb(artifact.totalSize)} GB 内存
                {systemRamGb != null &&
                  (systemRamGb >= estimateRamGb(artifact.totalSize)
                    ? "（本机满足）"
                    : "（本机可能不足）")}
              </>
            )}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">{renderAction()}</div>
      </div>
      {task && (task.state === "downloading" || task.state === "queued") && (
        <div className="mt-2 space-y-1">
          <Progress value={task.progress} />
          <p className="truncate text-[11px] text-text-muted">
            {task.currentFile ?? ""} · {formatBytes(task.bytesDownloaded)} /{" "}
            {formatBytes(task.totalBytes)}
          </p>
        </div>
      )}
    </div>
  );
}
