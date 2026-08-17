import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { AsrAdvancedParams } from "@/components/asr/AsrAdvancedParams";
import { AsrBasicConfig } from "@/components/asr/AsrBasicConfig";
import { AsrRunControl } from "@/components/asr/AsrRunControl";
import { AsrTechnicalInfo } from "@/components/asr/AsrTechnicalInfo";
import { AsrTestDialog } from "@/components/asr/AsrTestDialog";

/**
 * 语音识别（ASR）配置页：标题行含识别开关与状态 + 基础配置 + 模型信息 + 测试识别对话框。
 */
export function AsrPage() {
  const [testOpen, setTestOpen] = useState(false);

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
          语音识别（ASR）配置
        </h1>
        <AsrRunControl />
      </header>

      <AsrBasicConfig onTestOpen={() => setTestOpen(true)} />

      <AsrTechnicalInfo />

      <AsrAdvancedParams />

      <AsrTestDialog open={testOpen} onClose={() => setTestOpen(false)} />
    </div>
  );
}
