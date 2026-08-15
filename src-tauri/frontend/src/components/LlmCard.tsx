import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, Bot, CircleAlert, LoaderCircle, Send, Square, Upload } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useLlm } from "@/hooks/useLlm";
import { api } from "@/lib/tauri";

export function LlmCard() {
  const llm = useLlm();
  const [text, setText] = useState("");
  const [pickError, setPickError] = useState<string | null>(null);

  const shownError = llm.error ?? llm.configError ?? pickError;

  const pickModel = async () => {
    const path = await open({
      multiple: false,
      title: "选择 GGUF 模型",
      filters: [{ name: "GGUF", extensions: ["gguf"] }],
    });
    if (typeof path === "string") {
      setPickError(null);
      try {
        await api.setLlmModelPath({ path });
        await llm.refreshConfig();
      } catch (e) {
        setPickError(String(e));
      }
    }
  };

  const busy = llm.loading || llm.generating;

  const handleSend = () => {
    if (!llm.ready || llm.generating) return;
    const trimmed = text.trim();
    if (!trimmed) return;
    void llm.chat(trimmed);
    setText("");
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-4 w-4 text-muted-foreground" />
          本地 LLM
        </CardTitle>
        <CardDescription>用 llama.cpp 在本地运行大语言模型（GGUF）</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-2">
            <span className="truncate font-mono text-xs text-muted-foreground">
              {llm.config?.model_path ?? "未配置模型路径"}
            </span>
            <Button variant="outline" size="sm" onClick={pickModel}>
              <Upload className="h-4 w-4" />
              选择模型
            </Button>
          </div>

          <div className="flex flex-wrap gap-2">
            {llm.ready ? (
              <Button variant="destructive" onClick={llm.unload} disabled={busy}>
                <Square className="h-4 w-4" />
                卸载模型
              </Button>
            ) : (
              <Button onClick={llm.load} disabled={busy || !llm.config?.models_present}>
                <LoaderCircle className="h-4 w-4" />
                {llm.loading ? "加载中…" : "加载模型"}
              </Button>
            )}
          </div>

          <label className="flex items-center gap-2 text-sm text-muted-foreground">
            <input
              type="checkbox"
              className="h-4 w-4"
              checked={llm.config?.enable_thinking ?? false}
              onChange={(e) => void llm.setThinking(e.target.checked)}
            />
            开启思考模式（输出 &lt;think&gt; 块）
          </label>

          <label className="flex items-center gap-2 text-sm text-muted-foreground">
            <input
              type="checkbox"
              className="h-4 w-4"
              checked={llm.config?.auto_load ?? false}
              onChange={(e) => void llm.setAutoLoad(e.target.checked)}
            />
            启动时自动加载模型
          </label>
        </div>

        {llm.config && !llm.config.models_present && (
          <Alert variant="warning">
            <AlertTriangle className="h-4 w-4" />
            <AlertTitle>模型文件缺失</AlertTitle>
            <AlertDescription className="whitespace-pre-wrap">
              未找到 GGUF 模型文件（已自动扫描 ~/.zapmomo/models/）。 请下载模型（如
              Qwen3-4B-Instruct-2507 Q4_K_M），或点击上方「选择模型」手动指定 .gguf 文件。
            </AlertDescription>
          </Alert>
        )}

        <div className="space-y-2">
          <textarea
            className="w-full rounded-md border bg-muted/40 p-3 text-sm outline-none focus:ring-1 focus:ring-ring"
            rows={2}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
            placeholder="输入你的消息…（Enter 发送，Shift+Enter 换行）"
            disabled={!llm.ready}
          />
          <div className="flex flex-wrap gap-2">
            <Button onClick={handleSend} disabled={!llm.ready || llm.generating}>
              <Send className="h-4 w-4" />
              发送
            </Button>
            {llm.generating && (
              <Button variant="destructive" onClick={llm.stop}>
                <Square className="h-4 w-4" />
                停止
              </Button>
            )}
          </div>
        </div>

        {(llm.generating || llm.response) && (
          <div className="rounded-md border bg-muted/40 p-3">
            {llm.generating && <p className="mb-1 text-xs text-muted-foreground">生成中…</p>}
            <p className="whitespace-pre-wrap text-sm">{llm.response}</p>
          </div>
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
