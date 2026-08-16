import { Live2dCard } from "@/components/Live2dCard";

/** 伙伴页：暂时安置 Live2D 角色模型卡片（Live2D 属伙伴/角色能力，后续整合进完整伙伴页）。 */
export function CompanionPage() {
  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold tracking-tight text-text-primary">伙伴</h1>
      <Live2dCard />
    </div>
  );
}
