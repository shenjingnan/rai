import { AlertTriangle, CircleAlert, Download, Info } from "lucide-react";
import { Fragment } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { useRuntime } from "@/providers/RuntimeContext";
import type { KwsConfigInfo } from "@/types/tauri";

function ConfigList({ config }: { config: KwsConfigInfo }) {
  const rows = [
    { label: "模型目录", value: config.model_dir, mono: true },
    { label: "后端 / 线程", value: `${config.provider} / ${config.num_threads}` },
    { label: "采样率", value: String(config.sample_rate) },
    { label: "关键词", value: config.keywords.join("、") || "（空）", mono: true },
    { label: "配置路径", value: config.settings_path, mono: true },
  ];

  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2">
      {rows.map((row) => (
        <Fragment key={row.label}>
          <dt className="text-sm text-muted-foreground">{row.label}</dt>
          <dd className={row.mono ? "font-mono text-sm" : "text-sm"}>{row.value}</dd>
        </Fragment>
      ))}
    </dl>
  );
}

export function ConfigCard() {
  const { kws } = useRuntime();
  const { config, error } = kws.config;
  const { downloading, progress, error: downloadError, download } = kws.download;

  const percent =
    progress?.stage === "downloading" ? Math.max(0, Math.min(100, progress.percent)) : 100;
  const busy = downloading || (config?.model_downloading ?? false);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Info className="h-4 w-4 text-muted-foreground" />
          配置
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {error ? (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
          </Alert>
        ) : config ? (
          <ConfigList config={config} />
        ) : null}

        {config && !config.models_present && (
          <>
            <Alert variant="warning">
              <AlertTriangle className="h-4 w-4" />
              <AlertTitle>模型文件缺失</AlertTitle>
              <AlertDescription className="whitespace-pre-wrap">
                模型文件缺失（{config.model_dir}）。点击下方按钮下载后即可开始监听。
              </AlertDescription>
            </Alert>

            <div className="flex flex-col gap-2">
              <Button onClick={download} disabled={busy}>
                <Download className="h-4 w-4" />
                {busy ? "下载中…" : "下载模型（约 33MB）"}
              </Button>
              {progress && (
                <div className="space-y-1">
                  <Progress value={percent} />
                  <p className="text-xs text-muted-foreground">{progress.message}</p>
                </div>
              )}
            </div>
          </>
        )}

        {downloadError && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{downloadError}</AlertDescription>
          </Alert>
        )}
      </CardContent>
    </Card>
  );
}
