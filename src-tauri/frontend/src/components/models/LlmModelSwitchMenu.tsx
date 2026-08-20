import { FolderOpen } from "lucide-react";
import { useState } from "react";
import { LlmPresetDialog } from "@/components/llm/LlmPresetDialog";
import { isHttpProvider } from "@/components/llm/llmMeta";
import { Button } from "@/components/ui/button";
import { useRuntime } from "@/providers/RuntimeContext";

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/**
 * 摘要 LLM 行的模型快速切换：模型名文本 + 「选择模型」按钮（与 LLM 配置页
 * LlmCoreConfig 同款样式/文案/禁用逻辑），打开同一个选择模型弹窗。
 * 组件自带点击冒泡拦截（按钮/弹窗内点击均不触发所在行的 Link 导航）。
 */
export function LlmModelSwitchMenu() {
  const { llm } = useRuntime();
  const [open, setOpen] = useState(false);
  // HTTP API 模式走云端模型，本地切换无意义：只显示模型名。
  const isHttp = isHttpProvider(llm.config?.provider);
  // 与 LlmCoreConfig.pickDisabled 一致：加载中/生成中禁用（已加载可无缝换模）。
  const pickDisabled = llm.loading || llm.generating;

  return (
    // 拦截点击：stopPropagation 挡 react-router 的 JS 导航，preventDefault 取消浏览器
    // 「激活祖先 <a>」的原生默认行为（a 内嵌 button 时点击会跟随 href 整页跳转）。
    // biome-ignore lint/a11y/noStaticElementInteractions: 静态容器仅拦截鼠标冒泡，交互由内部按钮承载
    // biome-ignore lint/a11y/useKeyWithClickEvents: 仅拦截点击冒泡防误触导航，键盘交互由内部按钮处理
    <span
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
      }}
    >
      <span className="inline-flex min-w-0 items-center gap-1.5">
        <span className="truncate text-xs text-text-secondary">
          {basename(llm.config?.model_path ?? "")}
        </span>
        {!isHttp && (
          <Button
            size="sm"
            onClick={() => setOpen(true)}
            disabled={pickDisabled}
            aria-label="选择模型"
          >
            <FolderOpen className="h-4 w-4" />
            选择模型
          </Button>
        )}
      </span>
      {!isHttp && <LlmPresetDialog open={open} onClose={() => setOpen(false)} />}
    </span>
  );
}
