import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  AudioLines,
  CircleAlert,
  Download,
  Play,
  Square,
  Upload,
  Volume2,
} from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { CUSTOM_VOICE } from "@/hooks/useTts";
import { useRuntime } from "@/providers/RuntimeContext";

export function TtsCard() {
  const { tts } = useRuntime();
  const [text, setText] = useState("你好，我是 ZapMomo。");

  const synthPercent = Math.max(0, Math.min(100, (tts.progress ?? 0) * 100));
  const downloadPercent =
    tts.downloadProgress?.stage === "downloading"
      ? Math.max(0, Math.min(100, tts.downloadProgress.percent))
      : 100;
  const busy = tts.downloading || (tts.config?.model_downloading ?? false);
  const shownError = tts.error ?? tts.configError ?? tts.downloadError;
  const isCustom = tts.selectedVoice === CUSTOM_VOICE;

  const pickWav = async () => {
    const path = await open({
      multiple: false,
      title: "选择参考音频",
      filters: [{ name: "WAV", extensions: ["wav"] }],
    });
    if (typeof path === "string") {
      tts.setCustomWav(path);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Volume2 className="h-4 w-4 text-muted-foreground" />
          语音合成
        </CardTitle>
        <CardDescription>把文本合成为语音（ZipVoice 零样本声音克隆）</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <textarea
          className="w-full rounded-md border bg-muted/40 p-3 text-sm outline-none focus:ring-1 focus:ring-ring"
          rows={3}
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="输入要合成的文本"
        />

        <div className="space-y-2">
          <label className="text-sm text-muted-foreground" htmlFor="tts-voice">
            音色
          </label>
          <select
            id="tts-voice"
            className="w-full rounded-md border bg-muted/40 p-2 text-sm outline-none focus:ring-1 focus:ring-ring"
            value={tts.selectedVoice}
            onChange={(e) => tts.setSelectedVoice(e.target.value)}
            disabled={tts.voices.length === 0}
          >
            <option value="">默认音色</option>
            {tts.voices.map((v) => (
              <option key={v.id} value={v.id}>
                {v.name}
              </option>
            ))}
            <option value={CUSTOM_VOICE}>自定义…</option>
          </select>

          {isCustom && (
            <div className="space-y-2 rounded-md border bg-muted/40 p-3">
              <div className="flex flex-wrap items-center gap-2">
                <Button variant="outline" size="sm" onClick={pickWav}>
                  <Upload className="h-4 w-4" />
                  选择参考音频
                </Button>
                {tts.customWav && (
                  <>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={tts.transcribe}
                      disabled={tts.transcribing}
                    >
                      <AudioLines className="h-4 w-4" />
                      {tts.transcribing ? "转写中…" : "自动转写"}
                    </Button>
                    <span className="truncate font-mono text-xs text-muted-foreground">
                      {tts.customWav}
                    </span>
                  </>
                )}
              </div>
              <textarea
                className="w-full rounded-md border bg-background p-2 text-sm outline-none focus:ring-1 focus:ring-ring"
                rows={2}
                value={tts.customText ?? ""}
                onChange={(e) => tts.setCustomText(e.target.value || null)}
                placeholder="参考音频的逐字转写文本（点「自动转写」或手动填写，须与音频一致）"
              />
              {tts.transcribeError && (
                <p className="text-xs text-destructive">{tts.transcribeError}</p>
              )}
            </div>
          )}
        </div>

        <div className="flex flex-wrap gap-2">
          <Button onClick={() => tts.synthesize(text)} disabled={tts.synthesizing}>
            <Play className="h-4 w-4" />
            合成
          </Button>
          <Button variant="destructive" onClick={tts.stop} disabled={!tts.synthesizing}>
            <Square className="h-4 w-4" />
            停止
          </Button>
          {tts.audioUrl && (
            <Button variant="outline" onClick={tts.play}>
              <Volume2 className="h-4 w-4" />
              播放
            </Button>
          )}
        </div>

        {/* biome-ignore lint/a11y/useMediaCaption: 合成语音无字幕轨 */}
        <audio ref={tts.audioRef} src={tts.audioUrl ?? undefined} className="hidden" />

        {tts.synthesizing && (
          <div className="space-y-1">
            <Progress value={synthPercent} />
            <p className="text-xs text-muted-foreground">合成中 {synthPercent.toFixed(0)}%</p>
          </div>
        )}

        {tts.result && (
          <p className="text-xs text-muted-foreground">
            已生成音频（{tts.result.duration.toFixed(1)}s）
          </p>
        )}

        {tts.config && !tts.config.models_present && (
          <>
            <Alert variant="warning">
              <AlertTriangle className="h-4 w-4" />
              <AlertTitle>模型文件缺失</AlertTitle>
              <AlertDescription className="whitespace-pre-wrap">
                模型文件缺失（{tts.config.model_dir}）。下载后即可合成语音。
              </AlertDescription>
            </Alert>

            <div className="flex flex-col gap-2">
              <Button onClick={tts.download} disabled={busy}>
                <Download className="h-4 w-4" />
                {busy ? "下载中…" : "下载模型（约 164MB）"}
              </Button>
              {tts.downloadProgress && (
                <div className="space-y-1">
                  <Progress value={downloadPercent} />
                  <p className="text-xs text-muted-foreground">{tts.downloadProgress.message}</p>
                </div>
              )}
            </div>
          </>
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
