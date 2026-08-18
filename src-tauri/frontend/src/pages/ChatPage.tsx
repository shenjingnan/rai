import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { VoiceStatusBadge } from "@/components/voice/VoiceStatusBadge";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 语音对话页：实时字幕 + 流式回复 + 会话开关。
 *
 * 语音本身全在后端（`voice` 会话线程），页面只订阅 `voice-session-*` 事件做展示。
 */
export function ChatPage() {
  const { voice, kws, asr } = useRuntime();
  const kwsEnabled = kws.config.config?.enabled ?? false;
  const asrEnabled = asr.config.config?.enabled ?? false;
  const capabilitiesReady = kwsEnabled && asrEnabled;

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-3">
            <h1 className="text-xl font-semibold tracking-tight text-text-primary">语音对话</h1>
            <VoiceStatusBadge phase={voice.phase} running={voice.running} />
          </div>
          <p className="mt-0.5 text-sm text-muted-foreground">
            喊唤醒词开始对话，播报中喊唤醒词可打断
          </p>
        </div>
        <Switch
          checked={voice.running}
          onCheckedChange={(on) => (on ? void voice.start() : void voice.stop())}
          disabled={voice.pending || !capabilitiesReady}
          aria-label="语音对话开关"
        />
      </div>

      {!capabilitiesReady && (
        <Alert>
          <AlertTitle>语音互动未启用</AlertTitle>
          <AlertDescription>
            语音对话需要同时启用「唤醒词」(KWS) 与「语音识别」(ASR)。请在「模型与能力」页开启后使用。
          </AlertDescription>
        </Alert>
      )}

      {voice.error && (
        <Alert variant="destructive">
          <AlertTitle>语音会话异常</AlertTitle>
          <AlertDescription>{voice.error}</AlertDescription>
        </Alert>
      )}

      <Card className="flex min-h-0 flex-1 flex-col">
        <CardHeader>
          <CardTitle>实时对话</CardTitle>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-3 overflow-y-auto">
          {voice.userSegments.length === 0 && !voice.partial && !voice.replyText && (
            <p className="text-sm text-muted-foreground">
              {voice.running
                ? "待唤醒中，喊唤醒词开始对话…"
                : "打开开关后，喊唤醒词开始对话…"}
            </p>
          )}

          {voice.userSegments.map((seg) => (
            <div key={seg.id}>
              <p className="text-sm font-medium text-text-primary">
                你 <span className="text-xs font-normal text-text-muted">{seg.at}</span>
              </p>
              <p className="mt-0.5 text-sm text-text-primary">{seg.text}</p>
            </div>
          ))}

          {voice.partial && (
            <p className="text-sm italic text-muted-foreground">{voice.partial}</p>
          )}

          {(voice.replyText || voice.currentSentence) && (
            <div>
              <p className="text-sm font-medium text-text-primary">桌宠</p>
              <p className="mt-0.5 text-sm text-text-primary">{voice.replyText}</p>
              {voice.currentSentence && !voice.replyDone && (
                <p className="mt-1 text-xs text-violet-600">正在播报：{voice.currentSentence}</p>
              )}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
