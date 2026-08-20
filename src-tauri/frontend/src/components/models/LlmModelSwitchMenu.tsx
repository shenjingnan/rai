import { ArrowUpDown } from "lucide-react";
import { useState } from "react";
import { LlmPresetDialog } from "@/components/llm/LlmPresetDialog";
import { isHttpProvider } from "@/components/llm/llmMeta";
import { useRuntime } from "@/providers/RuntimeContext";

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/**
 * 摘要 LLM 行的模型快速切换：模型名文本 + 「切换」按钮打开选择模型弹窗
 * （与 LLM 配置页同款 LlmPresetDialog：下载预设 / 设为当前 / 导入 GGUF）。
 * 组件自带点击冒泡拦截（按钮/弹窗内点击均不触发所在行的 Link 导航）。
 */
export function LlmModelSwitchMenu() {
  const { llm } = useRuntime();
  const [open, setOpen] = useState(false);
  // HTTP API 模式走云端模型，本地切换无意义：只显示模型名。
  const isHttp = isHttpProvider(llm.config?.provider);

  return (
    // 拦截点击冒泡：按钮与弹窗（Portal）内的点击都不触发所在行的 Link 导航。
    // biome-ignore lint/a11y/noStaticElementInteractions: 静态容器仅拦截鼠标冒泡，交互由内部按钮承载
    // biome-ignore lint/a11y/useKeyWithClickEvents: 仅拦截点击冒泡防误触导航，键盘交互由内部按钮处理
    <span onClick={(e) => e.stopPropagation()}>
      <span className="inline-flex min-w-0 items-center gap-1.5">
        <span className="truncate text-xs text-text-secondary">
          {basename(llm.config?.model_path ?? "")}
        </span>
        {!isHttp && (
          <button
            type="button"
            aria-label="切换 AI 大脑模型"
            onClick={() => setOpen(true)}
            className="inline-flex h-6 shrink-0 items-center gap-1 rounded-md border border-panel-border bg-panel-background px-2 text-xs text-text-secondary transition-colors hover:bg-nav-hover hover:text-text-primary"
          >
            <ArrowUpDown className="h-3 w-3" />
            切换
          </button>
        )}
      </span>
      {!isHttp && <LlmPresetDialog open={open} onClose={() => setOpen(false)} />}
    </span>
  );
}
