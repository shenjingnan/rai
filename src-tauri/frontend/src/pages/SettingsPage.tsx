import { CircleAlert, Download, Settings2 } from "lucide-react";
import { useEffect, useState } from "react";
import { DeviceSelect } from "@/components/DeviceSelect";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useToast } from "@/components/ui/toast";
import { api } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 设置页：通用设置（麦克风来源 / 隐藏 Dock 图标）。
 * 麦克风来源为全局共享配置：KWS、ASR 等均使用同一 device，选择后持久化到 backend settings.toml。
 */
export function SettingsPage() {
  const {
    devices: { error: devicesError },
  } = useRuntime();
  const toast = useToast();
  const [hideDockIcon, setHideDockIcon] = useState<boolean | null>(null);

  // 模型下载源配置
  const [downloadSource, setDownloadSource] = useState("auto");
  const [mirrorUrl, setMirrorUrl] = useState("");
  const [endpointLoading, setEndpointLoading] = useState(true);
  const [endpointSaving, setEndpointSaving] = useState(false);

  useEffect(() => {
    void api
      .getHideDockIcon()
      .then(setHideDockIcon)
      .catch(() => setHideDockIcon(false));
  }, []);

  useEffect(() => {
    void api
      .catalogGetEndpoint()
      .then((e) => {
        setDownloadSource(e.downloadSource);
        setMirrorUrl(e.mirrorUrl);
      })
      .catch(() => {})
      .finally(() => setEndpointLoading(false));
  }, []);

  // 立即应用并持久化；失败时回滚到原值。
  const handleToggle = (hide: boolean) => {
    setHideDockIcon(hide);
    void api.setHideDockIcon({ hide }).catch(() => setHideDockIcon((prev) => !prev));
  };

  const saveEndpoint = async () => {
    if (downloadSource !== "huggingface" && !mirrorUrl.trim()) {
      toast.error("请填写镜像地址");
      return;
    }
    setEndpointSaving(true);
    try {
      await api.catalogSetEndpoint({
        catalogBase: "https://huggingface.co",
        downloadSource,
        mirrorUrl,
      });
      toast.success("下载源已保存");
    } catch (e) {
      toast.error(String(e));
    } finally {
      setEndpointSaving(false);
    }
  };

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold tracking-tight text-text-primary">设置</h1>

      <section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">
        <div className="px-3.5 py-2.5">
          <div className="flex items-center gap-2.5">
            <Settings2 className="h-4 w-4 shrink-0 text-text-secondary" />
            <div>
              <h2 className="text-base font-semibold text-text-primary">通用</h2>
              <p className="mt-0.5 text-xs text-text-muted">应用行为与系统集成</p>
            </div>
          </div>
        </div>

        <dl className="divide-y divide-divider">
          <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
            <div className="min-w-0">
              <dt className="text-sm text-text-primary">麦克风来源</dt>
              <dd className="mt-0.5 text-xs text-text-muted">
                用于唤醒词检测与语音识别的输入设备，选择后全局生效并被记忆。
              </dd>
            </div>
            <DeviceSelect />
          </div>

          <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
            <div className="min-w-0">
              <dt className="text-sm text-text-primary">隐藏应用图标</dt>
              <dd className="mt-0.5 text-xs text-text-muted">
                在 Dock / Cmd+Tab 中隐藏应用图标（仅 macOS）
              </dd>
            </div>
            <Switch
              aria-label="隐藏应用图标"
              checked={hideDockIcon ?? false}
              onCheckedChange={handleToggle}
            />
          </div>

          <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
            <div className="min-w-0">
              <dt className="text-sm text-text-primary">重启应用</dt>
              <dd className="mt-0.5 text-xs text-text-muted">关闭并重新启动 ZapMomo</dd>
            </div>
            <Button size="sm" onClick={() => void api.restartApp()}>
              重启
            </Button>
          </div>
        </dl>

        {devicesError && (
          <div className="px-3.5 pb-2">
            <Alert variant="destructive">
              <CircleAlert className="h-4 w-4" />
              <AlertDescription className="whitespace-pre-wrap">{devicesError}</AlertDescription>
            </Alert>
          </div>
        )}
      </section>

      {/* 模型下载 */}
      <section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">
        <div className="px-3.5 py-2.5">
          <div className="flex items-center gap-2.5">
            <Download className="h-4 w-4 shrink-0 text-text-secondary" />
            <div>
              <h2 className="text-base font-semibold text-text-primary">模型下载</h2>
              <p className="mt-0.5 text-xs text-text-muted">
                选择模型文件的下载来源；可自定义镜像地址（如 hf-mirror.com）
              </p>
            </div>
          </div>
        </div>

        <dl className="divide-y divide-divider">
          <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
            <div className="min-w-0">
              <dt className="text-sm text-text-primary">下载来源</dt>
              <dd className="mt-0.5 text-xs text-text-muted">
                自动 = 官方优先，失败回退镜像；仅官方 / 仅镜像
              </dd>
            </div>
            <Select
              value={downloadSource}
              onValueChange={setDownloadSource}
              disabled={endpointLoading}
            >
              <SelectTrigger className="h-9 w-44">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="auto">自动（官方 + 镜像）</SelectItem>
                <SelectItem value="huggingface">仅官方（Hugging Face）</SelectItem>
                <SelectItem value="mirror">仅镜像</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {downloadSource !== "huggingface" && (
            <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
              <div className="min-w-0">
                <dt className="text-sm text-text-primary">镜像地址</dt>
                <dd className="mt-0.5 text-xs text-text-muted">可填写任意镜像，如 hf-mirror.com</dd>
              </div>
              <Input
                value={mirrorUrl}
                onChange={(e) => setMirrorUrl(e.target.value)}
                placeholder="https://hf-mirror.com"
                disabled={endpointLoading}
                className="h-9 w-64"
              />
            </div>
          )}

          <div className="flex items-center justify-end gap-2 px-3.5 py-2.5">
            <Button
              size="sm"
              onClick={() => void saveEndpoint()}
              disabled={endpointLoading || endpointSaving}
            >
              保存下载源
            </Button>
          </div>
        </dl>
      </section>
    </div>
  );
}
