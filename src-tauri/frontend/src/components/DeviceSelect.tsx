import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 全局麦克风输入设备选择（Select + 刷新）：绑定 AppRuntimeProvider 的 device/setDevice。
 * 供「设置」页与 KWS 页等任意位置复用；选择结果全局生效（KWS/ASR 共享）并持久化到 backend settings.toml。
 */
export function DeviceSelect() {
  const {
    devices: { devices: deviceList, refresh },
    device,
    setDevice,
    anyListening,
  } = useRuntime();

  return (
    <div className="flex shrink-0 items-center gap-2">
      <Select
        value={device}
        onValueChange={setDevice}
        disabled={anyListening || deviceList.length === 0}
      >
        <SelectTrigger className="w-56" aria-label="麦克风来源">
          <SelectValue placeholder={deviceList.length === 0 ? "未找到输入设备" : "选择麦克风"} />
        </SelectTrigger>
        <SelectContent>
          {deviceList.map((d) => (
            <SelectItem key={d} value={d}>
              {d}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Button
        variant="outline"
        size="icon"
        onClick={() => void refresh()}
        disabled={anyListening}
        aria-label="刷新设备列表"
      >
        <RefreshCw className="h-4 w-4" />
      </Button>
    </div>
  );
}
