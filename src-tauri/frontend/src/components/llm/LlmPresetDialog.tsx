import { CircleAlert, Download, FolderOpen, Trash2 } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { LibraryDialog } from "@/components/library/LibraryDialog";
import { ModelConfirmDialog } from "@/components/library/LibraryDialogs";
import { useLlmModelPicker } from "@/components/llm/useLlmModelPicker";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { LLM_PRESETS, useLlmPresets } from "@/hooks/useLlmPresets";
import { useSmoothProgress } from "@/hooks/useSmoothProgress";
import { estimateRamGb, formatBytes } from "@/lib/catalog/quantization";
import { useRuntime } from "@/providers/RuntimeContext";
import type { LibraryModel } from "@/types/modelLibrary";

interface LlmPresetDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * 选择模型弹窗：内置推荐预设（未安装→下载；已安装→设为当前 / 卸载；当前→标记），
 * 附「导入 GGUF 文件」（自定义模型）与模型库引导。卸载确认框嵌套在此弹窗内。
 */
export function LlmPresetDialog({ open, onClose }: LlmPresetDialogProps) {
  const { llm } = useRuntime();
  const presets = useLlmPresets();
  const { pick, pickError } = useLlmModelPicker();
  const [confirmModel, setConfirmModel] = useState<LibraryModel | null>(null);
  const { downloading, currentId, progress, error } = llm.download;

  // verifying/done 阶段后端 percent=-1，直接喂 Progress 会异常，非 downloading 一律按 100
  const targetPercent =
    progress?.stage === "downloading" ? Math.max(0, Math.min(100, progress.percent)) : 100;
  // 平滑插值：消除高频进度事件造成的进度条抖动
  const percent = useSmoothProgress(targetPercent);

  return (
    <LibraryDialog open={open} onClose={onClose} title="选择模型" width="lg">
      <p className="text-xs text-text-muted">
        内置推荐预设：下载后自动配置并加载；已安装的模型可设为当前或卸载。
      </p>

      <div className="space-y-2">
        {LLM_PRESETS.map((p) => {
          // 仅「完整已安装」视为已安装（list_model_library 对未安装 registry 模型也返回记录）
          const installed =
            (presets.models ?? []).find(
              (m) => (m.id === p.id || m.repoId === p.id) && m.installState === "installed",
            ) ?? null;
          const busy = downloading && currentId === p.id;
          return (
            <div
              key={p.id}
              className="flex items-center justify-between gap-3 rounded-lg border border-panel-border px-3 py-2.5"
            >
              <div className="min-w-0">
                <p className="text-sm font-medium text-text-primary">{p.name}</p>
                <p className="mt-0.5 text-xs text-text-muted">
                  {`${formatBytes(p.sizeBytes)} · 约 ${estimateRamGb(p.sizeBytes)}GB 内存 · ${p.tagline}`}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {installed ? (
                  installed.current ? (
                    <span className="inline-flex items-center gap-1.5 text-xs text-emerald-600">
                      <span className="h-1.5 w-1.5 rounded-full bg-current" />
                      当前模型
                    </span>
                  ) : (
                    <>
                      <Button
                        size="sm"
                        onClick={() => void presets.setCurrent(installed.id)}
                        disabled={busy}
                      >
                        设为当前
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="shadow-none text-destructive hover:text-destructive"
                        onClick={() => setConfirmModel(installed)}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                        卸载
                      </Button>
                    </>
                  )
                ) : (
                  <Button
                    size="sm"
                    onClick={() => void presets.download(p.id)}
                    disabled={downloading}
                    aria-label={`下载${p.name}`}
                  >
                    <Download className="h-4 w-4" />
                    {busy ? "下载中…" : "下载"}
                  </Button>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {progress && (
        <div className="space-y-1">
          <Progress value={percent} />
          <p className="text-xs text-text-muted">{progress.message}</p>
        </div>
      )}

      {(error || pickError || presets.error) && (
        <div className="space-y-2">
          {error && (
            <Alert variant="destructive">
              <CircleAlert className="h-4 w-4" />
              <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
            </Alert>
          )}
          {pickError && (
            <Alert variant="destructive">
              <CircleAlert className="h-4 w-4" />
              <AlertDescription className="whitespace-pre-wrap">{pickError}</AlertDescription>
            </Alert>
          )}
          {presets.error && (
            <Alert variant="destructive">
              <CircleAlert className="h-4 w-4" />
              <AlertDescription className="whitespace-pre-wrap">
                读取模型列表失败：{presets.error}
              </AlertDescription>
            </Alert>
          )}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2 pt-1">
        <Button variant="outline" size="sm" className="shadow-none" onClick={() => void pick()}>
          <FolderOpen className="h-3.5 w-3.5" />
          导入 GGUF 文件
        </Button>
        <Link
          to="/models/library"
          onClick={onClose}
          className="text-xs text-text-secondary transition-colors hover:text-text-primary"
        >
          更多模型 → 模型库
        </Link>
      </div>

      <ModelConfirmDialog
        open={confirmModel !== null}
        model={confirmModel}
        onClose={() => setConfirmModel(null)}
        onConfirm={(m) => {
          setConfirmModel(null);
          void presets.remove(m.id);
        }}
      />
    </LibraryDialog>
  );
}
