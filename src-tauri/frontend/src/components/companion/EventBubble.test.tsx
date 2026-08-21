import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EventBubble } from "./EventBubble";

const { listenMock, eventHandlers } = vi.hoisted(() => {
  const handlers: Record<string, (payload: unknown) => void> = {};
  return {
    listenMock: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
      handlers[event] = (payload) => cb({ payload });
      return Promise.resolve(() => {});
    }),
    eventHandlers: handlers,
  };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

function emitSpeak(text: string) {
  act(() => {
    eventHandlers["dsh-speak"]?.({ text, event: { type: "task-finished", session_id: "s" } });
  });
}

async function renderReady() {
  render(<EventBubble />);
  await act(async () => {
    await Promise.resolve();
  });
}

describe("EventBubble", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-21T10:00:00Z"));
    vi.clearAllMocks();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("订阅 dsh-speak，收到事件渲染气泡，8 秒后自动消失", async () => {
    await renderReady();
    expect(listenMock).toHaveBeenCalledWith("dsh-speak", expect.any(Function));
    emitSpeak("任务搞定啦！");
    expect(screen.getByText("任务搞定啦！")).toBeTruthy();
    act(() => {
      vi.advanceTimersByTime(8500);
    });
    expect(screen.queryByText("任务搞定啦！")).toBeNull();
  });

  it("淡出窗 opacity 翻转：8s 内可见，最后 600ms opacity 置 0，随后消失", async () => {
    await renderReady();
    emitSpeak("淡出测试");
    expect(screen.getByText("淡出测试")).toBeTruthy();
    // 进入 600ms 淡出窗（剩余 400ms）：interval 强制推进重渲染，opacity 已翻转。
    act(() => {
      vi.advanceTimersByTime(7600);
    });
    const bubble = screen.getByText("淡出测试").closest("div[style]") as HTMLElement | null;
    expect(bubble?.style.opacity).toBe("0");
    // 越过 8s 到期点：气泡被裁剪。
    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(screen.queryByText("淡出测试")).toBeNull();
  });

  it("同时最多显示 2 条：第 3 条出现时最旧的让位", async () => {
    await renderReady();
    emitSpeak("一");
    emitSpeak("二");
    emitSpeak("三");
    expect(screen.queryByText("一")).toBeNull();
    expect(screen.getByText("二")).toBeTruthy();
    expect(screen.getByText("三")).toBeTruthy();
  });

  it("队列上限 3：第 4 条到达时最旧的出队", async () => {
    await renderReady();
    emitSpeak("一");
    emitSpeak("二");
    emitSpeak("三");
    emitSpeak("四");
    expect(screen.queryByText("一")).toBeNull();
    expect(screen.getByText("三")).toBeTruthy();
    expect(screen.getByText("四")).toBeTruthy();
  });
});
