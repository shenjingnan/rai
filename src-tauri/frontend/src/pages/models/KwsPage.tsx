import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { KwsAdvancedParams } from "@/components/kws/KwsAdvancedParams";
import { KwsBasicConfig } from "@/components/kws/KwsBasicConfig";
import { KwsRunControl } from "@/components/kws/KwsRunControl";
import { KwsTechnicalInfo } from "@/components/kws/KwsTechnicalInfo";
import { KwsTestDialog } from "@/components/kws/KwsTestDialog";

/** 唤醒词（KWS）配置页：标题行含监听开关 + 基础配置 + 模型信息 + 高级参数。 */
export function KwsPage() {
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
          唤醒词（KWS）配置
        </h1>
        <KwsRunControl />
      </header>

      <KwsBasicConfig onTestOpen={() => setTestOpen(true)} />

      <KwsTechnicalInfo />

      <KwsAdvancedParams />

      <KwsTestDialog open={testOpen} onClose={() => setTestOpen(false)} />
    </div>
  );
}
