import { useEffect, useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { api } from "@/lib/tauri";

export function SettingsPage() {
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
      <h1 className="text-xl font-semibold tracking-tight text-text-primary">设置</h1>

      <Card>
        <CardHeader>
          <CardTitle>通用</CardTitle>
          <CardDescription>应用行为与系统集成</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <label className="flex items-center gap-2 text-sm text-muted-foreground">
            <input
              type="checkbox"
              className="h-4 w-4"
              checked={hideDockIcon ?? false}
              onChange={(e) => handleToggle(e.target.checked)}
            />
            在 Dock / Cmd+Tab 中隐藏应用图标（仅 macOS）
          </label>
        </CardContent>
      </Card>
    </div>
  );
}
