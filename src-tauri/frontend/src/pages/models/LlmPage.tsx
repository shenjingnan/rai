import { ArrowLeft, Info } from "lucide-react";
import { Link } from "react-router-dom";
import { LlmAdvancedParams } from "@/components/llm/LlmAdvancedParams";
import { LlmCoreConfig } from "@/components/llm/LlmCoreConfig";
import { LlmRunControl } from "@/components/llm/LlmRunControl";
import { LlmSystemPrompt } from "@/components/llm/LlmSystemPrompt";
import { isHttpProvider } from "@/components/llm/llmMeta";
import { useLlmModelPicker } from "@/components/llm/useLlmModelPicker";
import { useRuntime } from "@/providers/RuntimeContext";

/** AI 大脑（LLM）配置页：标题行含运行开关与状态 + 基础配置 + 高级参数 + 系统提示词。 */
export function LlmPage() {
  const { pick, pickError } = useLlmModelPicker();
  const { llm } = useRuntime();
  const isHttp = isHttpProvider(llm.config?.provider);

  return (
    <div className="space-y-4">
      <Link
        to="/models"
        className="inline-flex items-center gap-1.5 text-sm text-text-secondary transition-colors hover:text-text-primary"
      >
        <ArrowLeft className="h-4 w-4" />
        模型与能力
      </Link>

      <header className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <h1 className="text-2xl font-semibold tracking-tight text-text-primary">
          AI 大脑（LLM）配置
        </h1>
        <LlmRunControl />
      </header>

      <LlmCoreConfig pick={pick} pickError={pickError} />

      <LlmSystemPrompt />
      <LlmAdvancedParams />

      {isHttp && (
        <section className="flex items-start gap-3 rounded-[16px] border border-panel-border bg-panel-background px-5 py-4">
          <Info className="mt-0.5 h-4 w-4 shrink-0 text-text-muted" />
          <p className="text-xs leading-relaxed text-text-secondary">
            当前 LLM 使用 OpenAI 兼容的远程服务。
          </p>
        </section>
      )}
    </div>
  );
}
