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
