import { CircleAlert, Mic, Play, RefreshCw, Square } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ListenCardProps {
  devices: string[];
  devicesError: string | null;
  onRefreshDevices: () => void;
  isListening: boolean;
  error: string | null;
  onStart: (device: string | null, keywords: string | null) => void;
  onStop: () => void;
}

export function ListenCard({
  devices,
  devicesError,
  onRefreshDevices,
  isListening,
  error,
  onStart,
  onStop,
}: ListenCardProps) {
  const [device, setDevice] = useState("");
  const [keywords, setKeywords] = useState("");

  const handleStart = () => onStart(device || null, keywords || null);
  const shownError = error ?? devicesError;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Mic className="h-4 w-4 text-muted-foreground" />
          实时监听
        </CardTitle>
        <CardDescription>选择麦克风并开始唤醒词监听</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-end gap-2">
          <div className="flex-1 space-y-2">
            <Label htmlFor="device">麦克风</Label>
            <Select
              value={device}
              onValueChange={setDevice}
              disabled={isListening || devices.length === 0}
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
            onClick={onRefreshDevices}
            disabled={isListening}
            aria-label="刷新设备列表"
          >
            <RefreshCw className="h-4 w-4" />
          </Button>
        </div>

        <div className="space-y-2">
          <Label htmlFor="keywords">附加关键词</Label>
          <Input
            id="keywords"
            value={keywords}
            onChange={(e) => setKeywords(e.target.value)}
            placeholder="可选：直接输入中文（如 你好小智），多个用 / 分隔"
            disabled={isListening}
          />
        </div>

        <div className="flex gap-2">
          <Button onClick={handleStart} disabled={isListening}>
            <Play className="h-4 w-4" />
            开始监听
          </Button>
          <Button variant="destructive" onClick={onStop} disabled={!isListening}>
            <Square className="h-4 w-4" />
            停止监听
          </Button>
        </div>

        {shownError && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{shownError}</AlertDescription>
          </Alert>
        )}
      </CardContent>
    </Card>
  );
}
