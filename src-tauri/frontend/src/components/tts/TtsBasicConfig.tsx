import { CircleAlert, Download, FolderOpen, Mic, Settings2, Volume2 } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useRuntime } from "@/providers/RuntimeContext";
import { modelNameFromDir } from "./ttsMeta";

interface TtsBasicConfigProps {
  onTestOpen: () => void;
  onManageVoices: () => void;
}

/**
 * 基础配置（macOS 设置行）：
 * 当前模型（名称 + 就绪/未下载 Badge + 可展开完整路径）+
 * 底部「下载模型（模型缺失时）/ 测试语音」操作按钮。
 */
export function TtsBasicConfig({ onTestOpen, onManageVoices }: TtsBasicConfigProps) {
  const { tts } = useRuntime();
  const {
    config,
    configError,
    downloading,
    downloadProgress,
    downloadError,
    download,
    voices,
    selectedVoice,
    setSelectedVoice,
  } = tts;
  const [showPath, setShowPath] = useState(false);

  const modelsPresent = config?.models_present ?? false;
  const enabled = config?.enabled ?? true;
  const modelPath = config?.model_dir ?? "";
  const modelName = modelNameFromDir(modelPath);

  const percent =
    downloadProgress?.stage === "downloading"
      ? Math.max(0, Math.min(100, downloadProgress.percent))
      : 100;
  const busy = downloading || (config?.model_downloading ?? false);

  return (
    <section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">
      <div className="px-3.5 py-2.5">
        <div className="flex items-center gap-2.5">
          <Settings2 className="h-4 w-4 shrink-0 text-text-secondary" />
          <div>
            <h2 className="text-base font-semibold text-text-primary">基础配置</h2>
            <p className="mt-0.5 text-xs text-text-muted">TTS 合成与声音设置</p>
          </div>
        </div>
      </div>

      {configError && (
        <div className="space-y-2 px-3.5 pb-2">
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">
              读取配置失败：{configError}
            </AlertDescription>
          </Alert>
        </div>
      )}

      {config && !config.models_present && (
        <div className="px-3.5 pb-2">
          <Alert variant="warning">
            <CircleAlert className="h-4 w-4" />
            <AlertTitle>模型文件缺失</AlertTitle>
            <AlertDescription className="whitespace-pre-wrap">
              模型文件缺失（{config.model_dir}）。点击下方「下载模型」按钮下载后即可测试语音合成。
            </AlertDescription>
          </Alert>
        </div>
      )}

      {config && !enabled && (
        <div className="px-3.5 pb-2">
          <Alert variant="warning">
            <CircleAlert className="h-4 w-4" />
            <AlertTitle>语音合成已关闭</AlertTitle>
            <AlertDescription className="whitespace-pre-wrap">
              语音合成已关闭，可在页面顶部开启后再测试。
            </AlertDescription>
          </Alert>
        </div>
      )}

      <dl>
        <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
          <dt className="shrink-0 text-sm text-text-primary">当前模型</dt>
          <dd className="min-w-0">
            <span className="flex items-center justify-end gap-1.5">
              {modelPath && (
                <button
                  type="button"
                  aria-label={showPath ? "隐藏模型路径" : "查看模型路径"}
                  title={modelPath}
                  onClick={() => setShowPath((v) => !v)}
                  className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-accent hover:text-text-primary"
                >
                  <FolderOpen className="h-3.5 w-3.5" />
                </button>
              )}
              <span
                className="truncate text-sm font-medium text-text-primary"
                title={modelName ?? undefined}
              >
                {modelName ?? "未知模型"}
              </span>
              <Badge
                variant="outline"
                className={
                  modelsPresent
                    ? "shrink-0 border-emerald-500/20 bg-emerald-500/10 text-emerald-600"
                    : "shrink-0 border-amber-500/20 bg-amber-500/10 text-amber-600"
                }
              >
                {modelsPresent ? "已就绪" : "未下载"}
              </Badge>
            </span>
            {showPath && modelPath && (
              <p className="mt-1 truncate font-mono text-xs text-text-muted" title={modelPath}>
                {modelPath}
              </p>
            )}
          </dd>
        </div>
      </dl>

      {/* 默认音色：所有 TTS 合成（测试 / 语音会话）默认用该音色，选即持久化 [tts].voice */}
      <dl>
        <div className="flex items-center justify-between gap-3.5 border-t border-divider px-3.5 py-2.5">
          <dt className="shrink-0 text-sm text-text-primary">默认音色</dt>
          <dd className="min-w-0">
            <Select
              value={selectedVoice}
              onValueChange={(v) => void setSelectedVoice(v)}
              disabled={voices.length === 0}
            >
              <SelectTrigger
                id="tts-default-voice"
                aria-label="默认音色"
                className="h-8 w-48"
              >
                <SelectValue placeholder="默认（内置 leijun）" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="">默认（内置 leijun）</SelectItem>
                {voices.map((v) => (
                  <SelectItem key={v.id} value={v.id}>
                    {v.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </dd>
        </div>
      </dl>

      <div className="flex flex-wrap gap-2 border-t border-divider px-3.5 py-2.5">
        {!modelsPresent && (
          <Button onClick={download} disabled={busy}>
            <Download className="h-4 w-4" />
            {busy ? "下载中…" : "下载模型"}
          </Button>
        )}
        <Button
          variant="secondary"
          className="shadow-none"
          disabled={!modelsPresent}
          onClick={onTestOpen}
        >
          <Volume2 className="h-4 w-4" />
          测试语音
        </Button>
        <Button variant="secondary" className="shadow-none" onClick={onManageVoices}>
          <Mic className="h-4 w-4" />
          音色管理
        </Button>
      </div>

      {downloadProgress && (
        <div className="space-y-1 px-3.5 pb-3">
          <Progress value={percent} />
          <p className="text-xs text-text-muted">{downloadProgress.message}</p>
        </div>
      )}

      {downloadError && (
        <div className="px-3.5 pb-3">
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{downloadError}</AlertDescription>
          </Alert>
        </div>
      )}
    </section>
  );
}
