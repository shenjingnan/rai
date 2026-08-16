import { open } from "@tauri-apps/plugin-dialog";
import { CircleAlert, FolderOpen, Sparkles } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Live2dStage } from "@/components/live2d/Live2dStage";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { api, toAssetUrl } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";

/** 预览区基准高度（与常驻窗口一致）。 */
const PREVIEW_BASE_HEIGHT = 480;
const PREVIEW_MIN_WIDTH = 120;
const PREVIEW_MAX_WIDTH = 520;
const PREVIEW_INITIAL_WIDTH = 360;

/** 角色（Live2D）卡片：选择模型目录并内嵌预览。 */
export function Live2dCard() {
  const { live2d } = useRuntime();
  const { config, error } = live2d.config;
  const { modelUrl, loading, error: loadError, load } = live2d.model;
  const [stageError, setStageError] = useState<string | null>(null);
  const [previewSize, setPreviewSize] = useState({
    width: PREVIEW_INITIAL_WIDTH,
    height: PREVIEW_BASE_HEIGHT,
  });
  const [percent, setPercent] = useState(100);
  /** 输入框草稿值（失焦时才提交，输入过程中不被 clamp 干扰）。 */
  const [draftPercent, setDraftPercent] = useState("100");

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

  // 恢复持久化的缩放比例，用于滑块/输入框初值。
  useEffect(() => {
    if (config?.window_scale) {
      setPercent(Math.round(config.window_scale * 100));
    }
  }, [config?.window_scale]);

  // 调节窗口缩放比例：clamp 到 [25, 200]，调后端设置并通知角色窗口。
  const handleScaleChange = useCallback((value: number) => {
    const clamped = Math.max(25, Math.min(200, Math.round(value)));
    setPercent(clamped);
    void api.setCompanionScale({ scale: clamped / 100 });
  }, []);

  // percent 变化时（滑块拖动 / 配置加载）同步输入框草稿值。
  useEffect(() => {
    setDraftPercent(String(percent));
  }, [percent]);

  // 输入框失焦时才解析并提交；空值或非法值恢复为当前比例。
  const handleInputBlur = useCallback(() => {
    const trimmed = draftPercent.trim();
    if (trimmed === "") {
      setDraftPercent(String(percent));
      return;
    }
    const value = Number(trimmed);
    if (Number.isFinite(value)) {
      handleScaleChange(value);
    } else {
      setDraftPercent(String(percent));
    }
  }, [draftPercent, percent, handleScaleChange]);

  // 模型加载后按角色宽高比自适应预览区宽度（高度固定，宽度 clamp 到卡片可用区域）。
  const handleModelMetrics = useCallback((metrics: { aspectRatio: number }) => {
    const height = PREVIEW_BASE_HEIGHT;
    let width = Math.round(height * metrics.aspectRatio);
    if (!Number.isFinite(width) || width <= 0) {
      width = PREVIEW_INITIAL_WIDTH;
    }
    width = Math.max(PREVIEW_MIN_WIDTH, Math.min(width, PREVIEW_MAX_WIDTH));
    setPreviewSize({ width, height });
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

        <div className="flex items-center gap-3">
          <span className="shrink-0 text-sm text-muted-foreground">窗口尺寸</span>
          <Slider
            value={[percent]}
            min={25}
            max={200}
            step={5}
            onValueChange={([v]) => handleScaleChange(v)}
            className="flex-1"
          />
          <div className="flex shrink-0 items-center gap-1">
            <Input
              type="text"
              inputMode="numeric"
              value={draftPercent}
              onChange={(e) => setDraftPercent(e.target.value)}
              onBlur={handleInputBlur}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.currentTarget.blur();
                }
              }}
              className="w-20 text-right"
            />
            <span className="text-sm text-muted-foreground">%</span>
          </div>
        </div>

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
          <div
            className="mx-auto overflow-hidden rounded-lg border bg-muted"
            style={{ width: previewSize.width, height: previewSize.height }}
          >
            <Live2dStage
              modelUrl={displayUrl}
              width={previewSize.width}
              height={previewSize.height}
              onError={handleStageError}
              onModelMetrics={handleModelMetrics}
            />
          </div>
        )}
      </CardContent>
    </Card>
  );
}
