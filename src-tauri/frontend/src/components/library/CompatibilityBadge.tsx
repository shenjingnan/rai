import { compatibilityLabel } from "@/lib/catalog/query";
import { cn } from "@/lib/utils";
import type { CompatibilityLevel } from "@/types/catalog";

const TONE: Record<CompatibilityLevel, string> = {
  verified: "border-emerald-200 bg-emerald-50 text-emerald-600",
  compatible: "border-blue-200 bg-blue-50 text-blue-600",
  possible: "border-amber-200 bg-amber-50 text-amber-600",
  unsupported: "border-red-200 bg-red-50 text-red-500",
};

/** 兼容性徽标（与本地状态徽标视觉独立）。
 * `verified` / `compatible` 不显示：默认列表只展示可用模型，可见即可用，无需额外标注。
 * 仅"待确认兼容"与"不兼容"（打开「显示全部模型」后可见）展示提示徽标。 */
export function CompatibilityBadge({ level }: { level: CompatibilityLevel }) {
  if (level === "compatible" || level === "verified") return null;
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-1.5 py-px text-[10px] font-medium",
        TONE[level],
      )}
    >
      {compatibilityLabel(level)}
    </span>
  );
}
