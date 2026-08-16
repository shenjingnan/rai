import { CircleAlert, Mic, RefreshCw } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useRuntime } from "@/providers/RuntimeContext";

/** 全局麦克风选择：唤醒词监听与语音识别共用的输入设备。 */
export function DeviceCard() {
  const {
    devices: { devices, error, refresh },
    device,
    setDevice,
    anyListening,
  } = useRuntime();

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Mic className="h-4 w-4 text-muted-foreground" />
          麦克风
        </CardTitle>
        <CardDescription>唤醒词监听与语音识别共用的输入设备</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-end gap-2">
          <div className="flex-1 space-y-2">
            <Label htmlFor="device">输入设备</Label>
            <Select
              value={device}
              onValueChange={setDevice}
              disabled={anyListening || devices.length === 0}
            >
              <SelectTrigger id="device">
                <SelectValue placeholder={devices.length === 0 ? "未找到输入设备" : "选择麦克风"} />
              </SelectTrigger>
              <SelectContent>
                {devices.map((d) => (
                  <SelectItem key={d} value={d}>
                    {d}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <Button
            variant="outline"
            size="icon"
            onClick={refresh}
            disabled={anyListening}
            aria-label="刷新设备列表"
          >
            <RefreshCw className="h-4 w-4" />
          </Button>
        </div>

        {error && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
          </Alert>
        )}
      </CardContent>
    </Card>
  );
}
