import { ChevronDown, CircleAlert, Save, SlidersHorizontal } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import type { LlmParams, LlmParamsPatch } from "@/types/tauri";
import { isHttpProvider } from "./llmMeta";

type ParamKey = keyof LlmParamsPatch;

const PARAM_KEYS: ParamKey[] = [
  "context_size",
  "temperature",
  "top_p",
  "max_tokens",
  "threads",
  "gpu_layers",
  "top_k",
  "min_p",
  "repeat_penalty",
  "seed",
  "batch_size",
];

/** 需重新加载模型才能生效的字段（load 时固化的引擎参数）。 */
const NEEDS_RELOAD: ParamKey[] = ["context_size", "batch_size", "threads", "gpu_layers"];

interface ParamMeta {
  label: string;
  hint?: string;
  kind: "slider" | "number";
  min: number;
  max: number;
  step: number;
  suffix?: string;
}

/** 参数元数据：前端预校验边界与后端 `LlmParamsPatch::apply_to` 一致（后端是权威）。 */
const PARAM_META: Record<ParamKey, ParamMeta> = {
  context_size: {
    label: "上下文大小",
    kind: "number",
    min: 256,
    max: 1_048_576,
    step: 128,
    suffix: "token",
    hint: "决定模型能记住的对话长度，越大占用内存越多。修改后自动重载生效。",
  },
  batch_size: {
    label: "批大小",
    kind: "number",
    min: 1,
    max: 8192,
    step: 1,
    suffix: "token",
    hint: "单次推理的 token 批大小，影响速度、不影响回答质量。修改后自动重载生效。",
  },
  max_tokens: {
    label: "最大生成 Tokens",
    kind: "number",
    min: 16,
    max: 262_144,
    step: 16,
    suffix: "token",
    hint: "单次回复最多生成的 token 数，越大回复越长、耗时越久。",
  },
  temperature: {
    label: "温度",
    kind: "slider",
    min: 0,
    max: 2,
    step: 0.05,
    hint: "越高回答越随机有创意，越低越稳定保守（0 = 总是选最可能的词）。",
  },
  top_p: {
    label: "Top-P",
    kind: "slider",
    min: 0,
    max: 1,
    step: 0.01,
    hint: "只从累计概率占比最高的词中采样，越小越保守、越大越多样。",
  },
  top_k: {
    label: "Top-K",
    kind: "number",
    min: 0,
    max: 500,
    step: 1,
    hint: "只从前 K 个最可能的词中采样，0 = 关闭该限制。",
  },
  min_p: {
    label: "Min-P",
    kind: "number",
    min: 0,
    max: 1,
    step: 0.01,
    hint: "过滤概率过低（低于最高词概率 × Min-P）的词，降低噪音。",
  },
  repeat_penalty: {
    label: "重复惩罚",
    kind: "number",
    min: 0,
    max: 3,
    step: 0.05,
    hint: "惩罚重复出现的词，值越大越少重复，1 = 关闭，小于 1 会鼓励重复。",
  },
  seed: {
    label: "随机种子",
    kind: "number",
    min: 0,
    max: 4_294_967_295,
    step: 1,
    hint: "固定后每次生成结果可复现，0 = 每次随机。",
  },
  threads: {
    label: "线程数",
    kind: "number",
    min: 0,
    max: 512,
    step: 1,
    suffix: "核",
    hint: "用于推理的 CPU 线程数，0 = 自动（物理核数-2）。修改后自动重载生效。",
  },
  gpu_layers: {
    label: "GPU 层数",
    kind: "number",
    min: -1,
    max: 1024,
    step: 1,
    suffix: "层",
    hint: "卸载到 GPU 加速的层数，-1 = 全部，0 = 纯 CPU。修改后自动重载生效。",
  },
};

function toDraft(params: LlmParams | undefined): Record<ParamKey, string> {
  const out = {} as Record<ParamKey, string>;
  for (const k of PARAM_KEYS) out[k] = params ? String(params[k]) : "";
  return out;
}

function parseDraft(draft: Record<ParamKey, string>): LlmParamsPatch | null {
  const patch = {} as LlmParamsPatch;
  for (const k of PARAM_KEYS) {
    const raw = draft[k].trim();
    if (raw === "") return null;
    const v = Number(raw);
    if (!Number.isFinite(v)) return null;
    (patch as Record<ParamKey, number>)[k] = v;
  }
  return patch;
}

function isPristine(
  draft: Record<ParamKey, string> | null,
  params: LlmParams | undefined,
): boolean {
  if (!draft || !params) return true;
  const patch = parseDraft(draft);
  if (!patch) return false; // 非法值视为已修改，允许点保存触发校验
  return PARAM_KEYS.every(
    (k) => Math.abs((patch as Record<ParamKey, number>)[k] - params[k]) < 1e-6,
  );
}

interface ParamRowProps {
  key_: ParamKey;
  value: string;
  onChange: (v: string) => void;
}

function ParamRow({ key_, value, onChange }: ParamRowProps) {
  const meta = PARAM_META[key_];
  const numeric = Number(value);
  const valid = value.trim() !== "" && Number.isFinite(numeric);
  // 用 text + inputMode 承载数字（Live2dCard 先例），避免 number 输入在浏览器里对
  // 小数中间态（如 "0."）的裁剪导致受控值抖动；范围校验在保存时统一做。
  const sharedInput = {
    type: "text" as const,
    inputMode: "decimal" as const,
    value,
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => onChange(e.target.value),
    "aria-label": meta.label,
  };

  return (
    <div className="flex items-start gap-4 px-3.5 py-2.5">
      <div className="min-w-0 flex-1">
        <p className="text-sm text-text-primary">{meta.label}</p>
        {meta.hint && <p className="mt-0.5 text-xs text-text-muted">{meta.hint}</p>}
      </div>
      {/* 固定宽度控件列 + 输入框右对齐：保证各行输入框右缘对齐（后缀用固定宽度槽，不顶开输入框） */}
      <div className="flex w-64 shrink-0 items-center justify-end gap-2.5 pt-0.5">
        {meta.kind === "slider" ? (
          <Slider
            value={[valid ? numeric : meta.min]}
            min={meta.min}
            max={meta.max}
            step={meta.step}
            onValueChange={([v]) => onChange(String(v))}
            className="min-w-0 flex-1"
            aria-label={meta.label}
          />
        ) : null}
        <div className="flex shrink-0 items-center gap-1">
          <Input {...sharedInput} className="w-20 text-right" />
          <span className="w-8 shrink-0 text-left text-xs text-text-muted">
            {meta.suffix ?? ""}
          </span>
        </div>
      </div>
    </div>
  );
}

/**
 * 高级参数：采样与运行参数（11 项），批量「保存」写 backend。
 * 草稿用字符串 map（Live2dCard 模式），点保存才 parse + 校验；温度/Top-P 用滑块 + 数字输入。
 */
export function LlmAdvancedParams() {
  const { llm } = useRuntime();
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<Record<ParamKey, string> | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const params = llm.config?.params;
  const http = isHttpProvider(llm.config?.provider);

  // hydrate：config 就绪时填充草稿；dirty 时保留用户编辑，否则随 config 同步
  useEffect(() => {
    if (!params) return;
    setDraft((prev) => (prev === null || isPristine(prev, params) ? toDraft(params) : prev));
  }, [params]);

  const pristine = isPristine(draft, params);

  const handleEdit = (k: ParamKey, v: string) => {
    setSaveError(null);
    setDraft((prev) => {
      if (!prev) return prev;
      return { ...prev, [k]: v };
    });
  };

  const handleSave = async () => {
    if (!draft) return;
    const patch = parseDraft(draft);
    if (!patch) {
      setSaveError("请将全部参数填写为有效数字");
      return;
    }
    for (const k of PARAM_KEYS) {
      const meta = PARAM_META[k];
      const v = (patch as Record<ParamKey, number>)[k];
      if (v < meta.min || v > meta.max) {
        setSaveError(`${meta.label} 需在 ${meta.min}~${meta.max} 之间`);
        return;
      }
    }
    setSaving(true);
    setSaveError(null);
    try {
      await llm.setParams(patch);
      // 若改动了「需重载」字段且模型已加载 → 主动重载使改动立即生效
      if (llm.ready && params) {
        const changed = PARAM_KEYS.filter(
          (k) => Math.abs((patch as Record<ParamKey, number>)[k] - params[k]) > 1e-6,
        );
        if (changed.some((k) => NEEDS_RELOAD.includes(k))) {
          await llm.load();
        }
      }
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <section className="rounded-[16px] border border-panel-border bg-panel-background">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger className="flex items-center justify-between gap-2 px-4 py-3 text-left">
          <span className="flex items-center gap-2.5">
            <SlidersHorizontal className="h-4 w-4 shrink-0 text-text-secondary" />
            <span>
              <h2 className="text-base font-semibold text-text-primary">高级参数</h2>
              <p className="mt-0.5 text-xs text-text-muted">采样与运行参数</p>
            </span>
          </span>
          <ChevronDown
            className={cn(
              "h-4 w-4 shrink-0 text-text-muted transition-transform",
              open && "rotate-180",
            )}
          />
        </CollapsibleTrigger>
        <CollapsibleContent className="border-t border-divider">
          <div>
            {PARAM_KEYS.map((k) => (
              <ParamRow
                key={k}
                key_={k}
                value={draft?.[k] ?? ""}
                onChange={(v) => handleEdit(k, v)}
              />
            ))}
          </div>

          {saveError && (
            <div className="px-3.5 pb-2.5">
              <Alert variant="destructive">
                <CircleAlert className="h-4 w-4" />
                <AlertDescription className="whitespace-pre-wrap">{saveError}</AlertDescription>
              </Alert>
            </div>
          )}

          <div className="flex flex-wrap items-center justify-between gap-2 px-3.5 py-2.5">
            <p className="text-xs text-text-muted">
              {http
                ? "部分参数（上下文大小/线程/GPU 层等）仅对本地 llama.cpp 生效。"
                : "温度、Top-P 等采样参数下次对话生效；上下文/线程/GPU 修改保存后会自动重新加载模型生效。"}
            </p>
            <Button
              size="sm"
              disabled={pristine || saving}
              onClick={handleSave}
              aria-label="保存参数"
            >
              <Save className="h-4 w-4" />
              保存
            </Button>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </section>
  );
}
