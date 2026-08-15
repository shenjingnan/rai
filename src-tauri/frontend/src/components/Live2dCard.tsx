import { open } from "@tauri-apps/plugin-dialog";
import { CircleAlert, FolderOpen, Sparkles } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { Live2dStage } from "@/components/live2d/Live2dStage";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useLive2dConfig } from "@/hooks/useLive2dConfig";
import { useLive2dModel } from "@/hooks/useLive2dModel";
import { toAssetUrl } from "@/lib/tauri";

/** 角色（Live2D）卡片：选择模型目录并内嵌预览。 */
export function Live2dCard() {
  const { config, error, refresh } = useLive2dConfig();
  const { modelUrl, loading, error: loadError, load } = useLive2dModel(refresh);
  const [stageError, setStageError] = useState<string | null>(null);

  // 启动时若已持久化了模型，恢复 asset:// URL（未通过 load 重新选择时）。
  const displayUrl = useMemo(() => {
    if (modelUrl) return modelUrl;
    if (config?.models_present && config.model_file) {
      return toAssetUrl(config.model_file);
    }
    return null;
  }, [modelUrl, config]);

  const handleStageError = useCallback((e: Error) => {
    setStageError(e.message);
  }, []);

  const pickDirectory = async () => {
    const dir = await open({ directory: true, title: "选择 Live2D 模型目录" });
    if (typeof dir === "string") {
      setStageError(null);
      await load(dir);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Sparkles className="h-4 w-4 text-muted-foreground" />
          角色（Live2D）
        </CardTitle>
        <CardDescription>选择本地模型目录，实时预览角色</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {error && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
          </Alert>
        )}

        <Button onClick={pickDirectory} disabled={loading}>
          <FolderOpen className="h-4 w-4" />
          {loading ? "加载中…" : "选择模型目录"}
        </Button>

        {config?.model_dir && (
          <p className="break-all font-mono text-xs text-muted-foreground">
            {config.model_dir}
            {config.format ? `（${config.format}）` : ""}
          </p>
        )}

        {loadError && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{loadError}</AlertDescription>
          </Alert>
        )}

        {stageError && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{stageError}</AlertDescription>
          </Alert>
        )}

        {displayUrl && (
          <div className="mx-auto h-[480px] w-[360px] overflow-hidden rounded-lg border bg-muted">
            <Live2dStage
              modelUrl={displayUrl}
              width={360}
              height={480}
              onError={handleStageError}
            />
          </div>
        )}
      </CardContent>
    </Card>
  );
}
