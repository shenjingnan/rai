import { FolderOpen, MessageSquare, Settings2 } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useRuntime } from "@/providers/RuntimeContext";
import { LlmTestDialog } from "./LlmTestDialog";
import { currentModelName } from "./llmMeta";

interface LlmCoreConfigProps {
  pick: () => Promise<void>;
  pickError: string | null;
}

/**
 * 左栏基础配置（macOS 设置行）：
 * 当前模型（名称 + 按需展开的完整路径）/ 启动时自动加载 / 思考模式 + 底部操作按钮。
 * 路径默认隐藏，点 FolderOpen 图标展开，hover 图标可看全路径。
 */
export function LlmCoreConfig({ pick, pickError }: LlmCoreConfigProps) {
  const { llm } = useRuntime();
  const [testOpen, setTestOpen] = useState(false);
  const [showPath, setShowPath] = useState(false);

  const busy = llm.loading || llm.generating;
  const modelName = currentModelName(llm.config);
  const modelPath = llm.config?.model_path ?? "";
  // 选择模型在已加载时也可用（pick 会静默触发 reload 无缝切换）；加载中/生成中禁用
  const pickDisabled = llm.loading || llm.generating;
  const testDisabled = !llm.ready || busy;

  return (
    <section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">
      <div className="px-3.5 py-2.5">
        <div className="flex items-center gap-2.5">
          <Settings2 className="h-4 w-4 shrink-0 text-text-secondary" />
          <div>
            <h2 className="text-base font-semibold text-text-primary">基础配置</h2>
            <p className="mt-0.5 text-xs text-text-muted">模型、加载与行为设置</p>
          </div>
        </div>
      </div>

      {(llm.configError || pickError) && (
        <div className="space-y-2 px-3.5 pb-2">
          {llm.configError && (
            <Alert variant="destructive">
              <AlertDescription className="whitespace-pre-wrap">
                读取配置失败：{llm.configError}
              </AlertDescription>
            </Alert>
          )}
          {pickError && (
            <Alert variant="destructive">
              <AlertDescription className="whitespace-pre-wrap">
                选择模型失败：{pickError}
              </AlertDescription>
            </Alert>
          )}
        </div>
      )}

      <dl>
        <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
          <dt className="shrink-0 text-sm text-text-primary">当前模型</dt>
          <dd className="min-w-0">
            <span className="flex items-center justify-end gap-1.5">
              {modelPath && (
                <button
                  type="button"
                  aria-label={showPath ? "隐藏模型路径" : "查看模型路径"}
                  title={modelPath}
                  onClick={() => setShowPath((v) => !v)}
                  className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-accent hover:text-text-primary"
                >
                  <FolderOpen className="h-3.5 w-3.5" />
                </button>
              )}
              <span
                className="truncate text-sm font-medium text-text-primary"
                title={modelName ?? undefined}
              >
                {modelName ?? "未选择模型"}
              </span>
            </span>
            {showPath && modelPath && (
              <p className="mt-1 truncate font-mono text-xs text-text-muted" title={modelPath}>
                {modelPath}
              </p>
            )}
          </dd>
        </div>

        <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
          <dt className="text-sm text-text-primary">启动时自动加载模型</dt>
          <Switch
            aria-label="启动时自动加载模型"
            checked={llm.config?.auto_load ?? false}
            onCheckedChange={(v) => void llm.setAutoLoad(v)}
            trackClass="bg-emerald-500"
          />
        </div>

        <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
          <div className="min-w-0">
            <dt className="text-sm text-text-primary">思考模式</dt>
            <dd className="mt-0.5 text-xs text-text-muted">
              开启后能提升模型输出表现，但降低响应速度，仅部分模型支持。
            </dd>
          </div>
          <Switch
            aria-label="思考模式"
            checked={llm.config?.enable_thinking ?? false}
            onCheckedChange={(v) => void llm.setThinking(v)}
            trackClass="bg-emerald-500"
          />
        </div>
      </dl>

      <div className="flex flex-wrap gap-2 border-t border-divider px-3.5 py-2.5">
        <Button onClick={() => void pick()} disabled={pickDisabled}>
          <FolderOpen className="h-4 w-4" />
          选择模型
        </Button>
        <Button variant="secondary" disabled={testDisabled} onClick={() => setTestOpen(true)}>
          <MessageSquare className="h-4 w-4" />
          测试模型
        </Button>
      </div>

      <LlmTestDialog open={testOpen} onClose={() => setTestOpen(false)} />
    </section>
  );
}
