import { Check, ChevronDown, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import type { TtsVoice } from "@/types/tauri";

interface TtsSpeakerSelectProps {
  /** 受控选中说话人 sid。 */
  value: number;
  /** 选择说话人（选即触发，由调用方持久化）。 */
  onValueChange: (sid: number) => void;
  /** 说话人列表（`list_tts_voices` 在 kokoro 模型下返回的 103 项）。 */
  speakers: TtsVoice[];
  /** 触发按钮 id（a11y）。 */
  id?: string;
  /** 触发按钮 aria-label。 */
  ariaLabel?: string;
  disabled?: boolean;
  className?: string;
}

/** 说话人分组（按官方 id 前缀推导：zf_ 中文女声 / zm_ 中文男声 / af_|bf_ 英文）。 */
function speakerGroup(id: string): string {
  if (id.startsWith("zf_")) return "中文女声";
  if (id.startsWith("zm_")) return "中文男声";
  return "英文";
}

/**
 * Kokoro 说话人选择器（103 项）：搜索过滤 + 分组列表。
 *
 * 项目无 Command/Combobox 组件，Radix Select 内嵌搜索框有焦点冲突，
 * 故自管理弹层：触发按钮 + 面板（搜索 + 分组滚动列表），外点/Esc 关闭。
 * 103 项 DOM 无需虚拟滚动，前端过滤足够。
 */
export function TtsSpeakerSelect({
  value,
  onValueChange,
  speakers,
  id,
  ariaLabel = "说话人",
  disabled = false,
  className,
}: TtsSpeakerSelectProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const rootRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  // 外点关闭
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  // Esc 关闭；打开时聚焦搜索框
  useEffect(() => {
    if (!open) return;
    searchRef.current?.focus();
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open]);

  // 打开时重置过滤；按名称/官方 id 过滤（大小写不敏感）
  useEffect(() => {
    if (open) setQuery("");
  }, [open]);

  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = q
      ? speakers.filter(
          (s) => s.name.toLowerCase().includes(q) || s.id.toLowerCase().includes(q),
        )
      : speakers;
    const order = ["中文女声", "中文男声", "英文"];
    const byGroup = new Map<string, TtsVoice[]>();
    for (const s of filtered) {
      const g = speakerGroup(s.id);
      byGroup.set(g, [...(byGroup.get(g) ?? []), s]);
    }
    return order
      .filter((g) => byGroup.has(g))
      .map((g) => ({ group: g, items: byGroup.get(g) ?? [] }));
  }, [speakers, query]);

  const selected = speakers.find((s) => s.sid === value);
  const selectedLabel = selected ? selected.name : `说话人 ${value}`;

  return (
    <div ref={rootRef} className={cn("relative", className)}>
      <button
        type="button"
        id={id}
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
        className={cn(
          "flex h-8 w-full items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-none transition-colors",
          "hover:bg-accent hover:text-accent-foreground",
          "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
          "disabled:cursor-not-allowed disabled:opacity-50",
        )}
      >
        <span className="truncate text-text-primary">{selectedLabel}</span>
        <ChevronDown className="h-4 w-4 shrink-0 opacity-50" />
      </button>

      {open && (
        <div
          className="absolute right-0 z-50 mt-1 w-72 overflow-hidden rounded-md border border-panel-border bg-panel-background shadow-lg"
          role="listbox"
          aria-label={ariaLabel}
        >
          <div className="flex items-center gap-2 border-b border-divider px-2.5 py-2">
            <Search className="h-3.5 w-3.5 shrink-0 text-text-muted" />
            <input
              ref={searchRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="搜索说话人（如 088 / maple）"
              className="w-full bg-transparent text-sm outline-none placeholder:text-text-muted"
            />
          </div>
          <div className="max-h-64 overflow-y-auto py-1">
            {groups.length === 0 && (
              <p className="px-3 py-2 text-xs text-text-muted">未找到匹配的说话人</p>
            )}
            {groups.map(({ group, items }) => (
              <div key={group}>
                <p className="px-3 pb-0.5 pt-1.5 text-xs font-medium text-text-muted">
                  {group}（{items.length}）
                </p>
                {items.map((s) => (
                  <button
                    key={s.id}
                    type="button"
                    role="option"
                    aria-selected={s.sid === value}
                    onClick={() => {
                      onValueChange(s.sid ?? 0);
                      setOpen(false);
                    }}
                    className={cn(
                      "flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left text-sm transition-colors",
                      "hover:bg-accent hover:text-accent-foreground",
                      s.sid === value && "font-medium text-text-primary",
                    )}
                  >
                    <span className="truncate">{s.name}</span>
                    <span className="flex shrink-0 items-center gap-1">
                      <span className="font-mono text-xs text-text-muted">{s.id}</span>
                      {s.sid === value && <Check className="h-3.5 w-3.5 text-emerald-600" />}
                    </span>
                  </button>
                ))}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
