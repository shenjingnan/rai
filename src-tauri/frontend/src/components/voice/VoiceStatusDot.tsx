import { cn } from "@/lib/utils";
import type { VoiceSessionPhase } from "@/types/tauri";

const PHASE_DOT: Record<VoiceSessionPhase, { className: string }> = {
  idle: { className: "bg-blue-500 animate-pulse" },
  armed: { className: "bg-emerald-500" },
  greeting: { className: "bg-violet-500 animate-pulse" },
  waiting_speech: { className: "bg-blue-500 animate-pulse" },
  listening: { className: "bg-blue-500 animate-pulse" },
  thinking: { className: "bg-amber-500 animate-pulse" },
  speaking: { className: "bg-violet-500 animate-pulse" },
};

/**
 * 桌宠窗口极简状态点：`running` 时显示（启动中显示蓝色脉冲，之后随阶段变色）。
 * `pointer-events-none` 不挡桌宠窗口的拖拽/右键。
 */
export function VoiceStatusDot({
  phase,
  running,
}: {
  phase: VoiceSessionPhase;
  running: boolean;
}) {
  if (!running) return null;
  const meta = PHASE_DOT[phase];
  return (
    <span
      title={phase}
      className={cn(
        "pointer-events-none block h-2.5 w-2.5 rounded-full shadow-md",
        meta.className,
      )}
    />
  );
}
