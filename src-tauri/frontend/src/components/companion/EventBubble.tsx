import { useEffect, useRef, useState } from "react";
import { onDshSpeak } from "@/lib/tauri";
import type { DshSpeakPayload } from "@/types/tauri";

/** 气泡自动消失时间 */
const VISIBLE_MS = 8000;
/** 保留队列上限（超出丢最旧） */
const MAX_QUEUE = 3;
/** 同时展示上限（更旧的让位） */
const MAX_BUBBLES = 2;
/** 过期裁剪轮询周期（淡出动画的驱动源） */
const PRUNE_INTERVAL_MS = 500;

interface Bubble {
  id: number;
  text: string;
  /** 到期时间戳（Date.now() + VISIBLE_MS） */
  until: number;
}

/**
 * 桌宠事件气泡：订阅 `dsh-speak`，在角色上方展示模板台词。
 *
 * - 队列上限 3、同时最多 2 条、每条 8s 自动淡出（500ms 轮询裁剪，fake timers 可测）
 * - 最多两行截断（`line-clamp-2`；全文有对话记录兜底）
 * - `pointer-events-none`：不挡窗口拖动/右键/滚轮
 */
export function EventBubble() {
  const [bubbles, setBubbles] = useState<Bubble[]>([]);
  const nextIdRef = useRef(1);

  useEffect(() => {
    const unlisten = onDshSpeak(({ text }: DshSpeakPayload) => {
      const id = nextIdRef.current++;
      setBubbles((prev) =>
        [...prev, { id, text, until: Date.now() + VISIBLE_MS }].slice(-MAX_QUEUE),
      );
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // 定期裁剪过期气泡（interval 依赖 bubbles：空列表不挂定时器）
  useEffect(() => {
    if (bubbles.length === 0) return;
    const timer = setInterval(() => {
      const now = Date.now();
      setBubbles((prev) =>
        prev.some((b) => b.until <= now) ? prev.filter((b) => b.until > now) : prev,
      );
    }, PRUNE_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [bubbles]);

  const visible = bubbles.slice(-MAX_BUBBLES);
  const now = Date.now();

  return (
    <div
      data-testid="dsh-bubbles"
      className="pointer-events-none absolute inset-x-0 top-6 z-10 flex flex-col items-center gap-1 px-4"
    >
      {visible.map((b) => (
        <div
          key={b.id}
          className="max-w-[240px] rounded-2xl bg-black/70 px-3 py-1.5 text-center text-xs leading-relaxed text-white shadow-lg backdrop-blur-sm"
          style={{ opacity: b.until - now < 600 ? 0 : 1, transition: "opacity 500ms" }}
        >
          <span className="line-clamp-2">{b.text}</span>
        </div>
      ))}
    </div>
  );
}
