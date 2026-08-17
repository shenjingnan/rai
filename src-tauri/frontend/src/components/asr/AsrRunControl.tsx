import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { ASR_STATUS_COLOR, asrStatus } from "./asrMeta";

/**
 * 标题行右侧的运行控制：状态点 + 识别开关。
 * 开关真实绑定 `asr.listening.isListening`：ON→start_asr_listen(device)，OFF→stop_asr_listen。
 * ASR 无持久化 enabled，Switch 直接反映运行时识别状态。
 * 在途状态唯一使用共享的 `asr.listening.pending`（与 TestDialog 同一份），不另设本地 toggling。
 */
export function AsrRunControl() {
  const { asr, device } = useRuntime();
  const configured = asr.config.config?.models_present ?? false;
  const { isListening, pending } = asr.listening;
  const status = asrStatus(asr.listening);

  const handleToggle = (on: boolean) => {
    if (on) void asr.listening.start(device || null);
    else void asr.listening.stop();
  };

  // 模型缺失时若已在识别仍允许关掉开关；否则禁用防重复点击与无效启动
  const disabled = pending || (!configured && !isListening);

  return (
    <div className="flex items-center gap-2.5">
      <span
        className={cn(
          "inline-flex items-center gap-1.5 text-sm font-medium",
          ASR_STATUS_COLOR[status.tone],
        )}
      >
        <span className="h-1.5 w-1.5 rounded-full bg-current" />
        {status.label}
      </span>
      <Switch
        aria-label="语音识别开关"
        checked={isListening}
        onCheckedChange={handleToggle}
        disabled={disabled}
        trackClass="bg-emerald-500"
      />
    </div>
  );
}
