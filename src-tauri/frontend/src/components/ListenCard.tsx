import { BellRing, CircleAlert, Play, Square } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface ListenCardProps {
  isListening: boolean;
  error: string | null;
  onStart: (keywords: string | null) => void;
  onStop: () => void;
}

export function ListenCard({ isListening, error, onStart, onStop }: ListenCardProps) {
  const [keywords, setKeywords] = useState("");

  const handleStart = () => onStart(keywords || null);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <BellRing className="h-4 w-4 text-muted-foreground" />
          实时监听
        </CardTitle>
        <CardDescription>说出唤醒词触发反应</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
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
