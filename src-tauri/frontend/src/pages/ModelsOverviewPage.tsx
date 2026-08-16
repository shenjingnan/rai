import { BellRing, Bot, Database, Subtitles, Volume2 } from "lucide-react";
import type { ComponentType } from "react";
import { Link } from "react-router-dom";
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

interface CapabilityEntry {
  to: string;
  icon: ComponentType<{ className?: string }>;
  name: string;
  desc: string;
}

const CAPABILITIES: CapabilityEntry[] = [
  { to: "/models/kws", icon: BellRing, name: "唤醒词（KWS）", desc: "关键词唤醒检测" },
  { to: "/models/asr", icon: Subtitles, name: "语音识别（ASR）", desc: "实时转写麦克风语音" },
  { to: "/models/llm", icon: Bot, name: "本地大模型（LLM）", desc: "用 llama.cpp 本地推理" },
  { to: "/models/tts", icon: Volume2, name: "语音合成（TTS）", desc: "ZipVoice 声音克隆" },
  { to: "/models/library", icon: Database, name: "模型库", desc: "本地模型文件管理" },
];

/** 模型概览页骨架：能力入口列表，点击进入对应独立详情页。 */
export function ModelsOverviewPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-xl font-semibold tracking-tight text-text-primary">模型与能力</h1>
      <div className="grid gap-4 sm:grid-cols-2">
        {CAPABILITIES.map((c) => (
          <Link key={c.to} to={c.to}>
            <Card className="h-full transition-colors hover:border-primary/40">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <c.icon className="h-4 w-4 text-muted-foreground" />
                  {c.name}
                </CardTitle>
                <CardDescription>{c.desc}</CardDescription>
              </CardHeader>
            </Card>
          </Link>
        ))}
      </div>
    </div>
  );
}
