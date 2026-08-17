import { AudioWaveform, ChevronDown } from "lucide-react";
import { useState } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 模型信息（默认展开）：运行时 / 执行 Provider / 线程数 / 采样率 / 识别模式 /
 * 支持语言 / 标点模型 / 模型目录 / 配置路径。
 * 全部来自 get_asr_config 的只读字段，无任何可编辑项。
 * 注意：ASR 运行时固定为 sherpa-onnx；config.provider（cpu）是执行 Provider/后端，两者概念不同。
 */
export function AsrTechnicalInfo() {
  const { asr } = useRuntime();
  const [open, setOpen] = useState(true);
  const config = asr.config.config;

  return (
    <section className="rounded-[16px] border border-panel-border bg-panel-background">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger className="flex items-center justify-between gap-2 px-4 py-3 text-left">
          <span className="flex items-center gap-2.5">
            <AudioWaveform className="h-4 w-4 shrink-0 text-text-secondary" />
            <span>
              <h2 className="text-base font-semibold text-text-primary">模型信息</h2>
              <p className="mt-0.5 text-xs text-text-muted">运行时、执行 Provider、识别模式等</p>
            </span>
          </span>
          <ChevronDown
            className={cn(
              "h-4 w-4 shrink-0 text-text-muted transition-transform",
              open && "rotate-180",
            )}
          />
        </CollapsibleTrigger>
        <CollapsibleContent className="border-t border-divider">
          {config && (
            <dl>
              {/* 固定运行时：ASR 在 ZapMomo 中始终使用 sherpa-onnx */}
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">运行时</dt>
                <dd className="truncate text-sm text-text-secondary">sherpa-onnx</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">执行 Provider</dt>
                <dd className="truncate text-sm text-text-secondary">{config.provider}</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">线程数</dt>
                <dd className="truncate text-sm text-text-secondary">{config.num_threads}</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">采样率</dt>
                <dd className="truncate text-sm text-text-secondary">{config.sample_rate}</dd>
              </div>
              {/* 固定模型：streaming-zipformer 为流式架构，不可动态切换 */}
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">识别模式</dt>
                <dd className="truncate text-sm text-text-secondary">流式</dd>
              </div>
              {/* 固定模型：中英双语，引擎自动识别，无语言选择 */}
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">支持语言</dt>
                <dd className="truncate text-sm text-text-secondary">中文、English</dd>
              </div>
              {/* 仅表示标点模型文件存在/可用；引擎对最终结果自动加标点，无用户可控开关 */}
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">标点模型</dt>
                <dd className="truncate text-sm text-text-secondary">
                  {config.punctuation_present ? "已就绪" : "未安装"}
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">模型目录</dt>
                <dd className="truncate text-sm text-text-secondary">{config.model_dir}</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">配置路径</dt>
                <dd className="truncate text-sm text-text-secondary">{config.settings_path}</dd>
              </div>
            </dl>
          )}
        </CollapsibleContent>
      </Collapsible>
    </section>
  );
}
