import { AlertTriangle, CircleAlert, Download, Play, Square, Subtitles } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { useAsrConfig } from "@/hooks/useAsrConfig";
import { useAsrModelDownload } from "@/hooks/useAsrModelDownload";
import { useAsrResults } from "@/hooks/useAsrResults";

interface AsrCardProps {
  isListening: boolean;
  error: string | null;
  onStart: () => void;
  onStop: () => void;
}

export function AsrCard({ isListening, error, onStart, onStop }: AsrCardProps) {
  const config = useAsrConfig();
  const results = useAsrResults();
  const {
    downloading,
    progress,
    error: downloadError,
    download,
  } = useAsrModelDownload(config.refresh);

  const percent =
    progress?.stage === "downloading" ? Math.max(0, Math.min(100, progress.percent)) : 100;
  const busy = downloading || (config.config?.model_downloading ?? false);
  const shownError = error ?? config.error;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Subtitles className="h-4 w-4 text-muted-foreground" />
          语音识别
        </CardTitle>
        <CardDescription>实时转写麦克风语音</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <Button onClick={onStart} disabled={isListening}>
            <Play className="h-4 w-4" />
            开始识别
          </Button>
          <Button variant="destructive" onClick={onStop} disabled={!isListening}>
            <Square className="h-4 w-4" />
            停止识别
          </Button>
        </div>

        {/* 实时字幕 */}
        <div className="min-h-16 rounded-md border bg-muted/40 p-3">
          {results.partial ? (
            <p className="text-sm">{results.partial}</p>
          ) : (
            <p className="text-sm text-muted-foreground">
              {isListening ? "聆听中…" : "点击「开始识别」后，这里会实时显示转写文本"}
            </p>
          )}
        </div>

        {results.segments.length > 0 && (
          <ul className="max-h-40 overflow-y-auto">
            {results.segments.map((s) => (
              <li key={s.id} className="border-b py-1.5 text-sm last:border-b-0">
                [{s.at}] {s.text}
              </li>
            ))}
          </ul>
        )}

        {config.config && !config.config.models_present && (
          <>
            <Alert variant="warning">
              <AlertTriangle className="h-4 w-4" />
              <AlertTitle>模型文件缺失</AlertTitle>
              <AlertDescription className="whitespace-pre-wrap">
                模型文件缺失（{config.config.model_dir}）。下载后即可开始识别。
              </AlertDescription>
            </Alert>

            <div className="flex flex-col gap-2">
              <Button onClick={download} disabled={busy}>
                <Download className="h-4 w-4" />
                {busy ? "下载中…" : "下载模型（约 500MB）"}
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

        {shownError && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{shownError}</AlertDescription>
          </Alert>
        )}
      </CardContent>
    </Card>
  );
}
