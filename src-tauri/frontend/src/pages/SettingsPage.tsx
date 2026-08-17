import { CircleAlert, Settings2 } from "lucide-react";
import { useEffect, useState } from "react";
import { DeviceSelect } from "@/components/DeviceSelect";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Switch } from "@/components/ui/switch";
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
  const [hideDockIcon, setHideDockIcon] = useState<boolean | null>(null);

  useEffect(() => {
    void api
      .getHideDockIcon()
      .then(setHideDockIcon)
      .catch(() => setHideDockIcon(false));
  }, []);

  // 立即应用并持久化；失败时回滚到原值。
  const handleToggle = (hide: boolean) => {
    setHideDockIcon(hide);
    void api.setHideDockIcon({ hide }).catch(() => setHideDockIcon((prev) => !prev));
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
    </div>
  );
}
