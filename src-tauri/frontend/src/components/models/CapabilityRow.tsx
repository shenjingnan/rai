import { CircleHelp, type LucideIcon } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

export type CapabilityAccent = "violet" | "blue" | "green" | "orange";

const ACCENTS: Record<CapabilityAccent, { icon: string }> = {
  violet: { icon: "bg-violet-100 text-violet-600" },
  blue: { icon: "bg-blue-100 text-blue-600" },
  green: { icon: "bg-emerald-100 text-emerald-600" },
  orange: { icon: "bg-amber-100 text-amber-600" },
};

export type StatusTone = "on" | "off" | "always" | "loading" | "error";

const STATUS_COLOR: Record<StatusTone, string> = {
  on: "text-emerald-600",
  always: "text-emerald-600",
  loading: "text-blue-600",
  error: "text-red-600",
  off: "text-text-muted",
};

interface CapabilityRowProps {
  accent: CapabilityAccent;
  icon: LucideIcon;
  name: string;
  /** 简短英文代码，如 ASR / KWS / LLM */
  code?: string;
  description: string;
  statusText: string;
  statusTone: StatusTone;
  /** 提供 onToggle 才显示开关 */
  toggled?: boolean;
  onToggle?: () => void;
  toggleDisabled?: boolean;
  /** 开关禁用原因（底部小字展示） */
  toggleHint?: string;
  /** “?” 说明（原生 title 提示） */
  tooltip?: string;
}

/** AI 能力链路中的单个能力条目（两行迷你卡片）。 */
export function CapabilityRow({
  accent,
  icon: Icon,
  name,
  code,
  description,
  statusText,
  statusTone,
  toggled,
  onToggle,
  toggleDisabled,
  toggleHint,
  tooltip,
}: CapabilityRowProps) {
  const accentStyle = ACCENTS[accent];
  return (
    <div className="py-2">
      <div className="flex items-center gap-2.5">
        <span
          className={cn(
            "flex h-8 w-8 shrink-0 items-center justify-center rounded-full",
            accentStyle.icon,
          )}
        >
          <Icon className="h-4 w-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="truncate text-sm font-medium text-text-primary">{name}</span>
            {code && <span className="shrink-0 text-[11px] text-text-muted">{code}</span>}
            {tooltip && (
              <span title={tooltip} className="inline-flex shrink-0">
                <CircleHelp className="h-3.5 w-3.5 text-text-muted" />
              </span>
            )}
          </div>
          <p className="truncate text-xs text-text-secondary">{description}</p>
        </div>
      </div>

      <div className="mt-1.5 flex items-center justify-between gap-2 pl-10.5">
        <span className={cn("whitespace-nowrap text-xs", STATUS_COLOR[statusTone])}>
          {statusText}
        </span>
        {onToggle && (
          <Switch
            checked={toggled ?? false}
            onCheckedChange={onToggle}
            disabled={toggleDisabled}
            trackClass="bg-emerald-500"
            aria-label={`${name}开关`}
          />
        )}
      </div>
      {toggleHint && (
        <p className="mt-1 pl-10.5 text-[11px] leading-4 text-amber-600">{toggleHint}</p>
      )}
    </div>
  );
}
