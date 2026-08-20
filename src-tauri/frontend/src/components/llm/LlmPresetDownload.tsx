import { CircleAlert, Download, Sparkles } from "lucide-react";
import { Link } from "react-router-dom";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { useRuntime } from "@/providers/RuntimeContext";
import { estimateRamGb, formatBytes } from "@/lib/catalog/quantization";

/** 一键下载预设（id = models/model_registry.json 的 registry id；体积与 manifest size_bytes 对齐） */
const PRESETS = [
  {
    id: "qwen3-0.6b-q4-k-m",
    label: "快速体验",
    model: "Qwen3-0.6B · Q4_K_M",
    sizeBytes: 396_705_472,
    desc: "轻量，下载快，适合入门设备",
  },
  {
    id: "qwen3-4b-instruct-2507-q4-k-m",
    label: "更佳对话",
    model: "Qwen3-4B-Instruct-2507 · Q4_K_M",
    sizeBytes: 2_497_281_120,
    desc: "对话质量更好，需要较强硬件",
  },
] as const;

/**
 * 未配置模型时的一键下载区：两个预设（快速体验 / 更佳对话）+ 体积与内存估算，
 * 下载完成由 hook 自动写配置并加载；「更多模型」引导到模型库。
 * models_present 转 true 后由 LlmPage 卸载本组件。
 */
export function LlmPresetDownload() {
  const { llm } = useRuntime();
  const { downloading, currentId, progress, error, download } = llm.download;
  // verifying/done 阶段后端 percent=-1，直接喂 Progress 会异常，非 downloading 一律按 100
  const percent =
    progress?.stage === "downloading"
      ? Math.max(0, Math.min(100, progress.percent))
      : 100;

  return (
    <section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">
      <div className="px-3.5 py-2.5">
        <div className="flex items-center gap-2.5">
          <Sparkles className="h-4 w-4 shrink-0 text-text-secondary" />
          <div>
            <h2 className="text-base font-semibold text-text-primary">一键下载默认模型</h2>
            <p className="mt-0.5 text-xs text-text-muted">
              尚未配置模型。选择一个预设下载，完成后自动配置并加载。
            </p>
          </div>
        </div>
      </div>

      <dl className="divide-y divide-divider border-t border-divider">
        {PRESETS.map((p) => (
          <div
            key={p.id}
            className="flex items-center justify-between gap-3.5 px-3.5 py-2.5"
          >
            <div className="min-w-0">
              <dt className="text-sm text-text-primary">
                {p.label} · {p.model}
              </dt>
              <dd className="mt-0.5 text-xs text-text-muted">
                {formatBytes(p.sizeBytes)} · 约 {estimateRamGb(p.sizeBytes)}GB 内存 · {p.desc}
              </dd>
            </div>
            <Button
              onClick={() => void download(p.id)}
              disabled={downloading}
              className="shrink-0"
            >
              <Download className="h-4 w-4" />
              {downloading && currentId === p.id ? "下载中…" : "下载"}
            </Button>
          </div>
        ))}
      </dl>

      {progress && (
        <div className="space-y-1 border-t border-divider px-3.5 py-2.5">
          <Progress value={percent} />
          <p className="text-xs text-text-muted">{progress.message}</p>
        </div>
      )}

      {error && (
        <div className="border-t border-divider px-3.5 py-2.5">
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
          </Alert>
        </div>
      )}

      <div className="border-t border-divider px-3.5 py-2.5">
        <Link
          to="/models/library"
          className="text-xs text-text-secondary transition-colors hover:text-text-primary"
        >
          需要其他模型？前往模型库 →
        </Link>
      </div>
    </section>
  );
}
