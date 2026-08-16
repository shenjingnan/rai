import { CapabilityChain } from "@/components/models/CapabilityChain";
import { ModeBanner } from "@/components/models/ModeBanner";
import { ModelSummary } from "@/components/models/ModelSummary";

/** 模型概览页：AI 能力链路 + 当前模式提示 + 模型摘要。 */
export function ModelsOverviewPage() {
  return (
    <div className="flex flex-col gap-3">
      <header>
        <h1 className="text-2xl font-semibold tracking-tight text-text-primary">模型与能力</h1>
      </header>
      <CapabilityChain />
      <ModeBanner />
      <ModelSummary />
    </div>
  );
}
