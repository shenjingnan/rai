import { act, fireEvent, render, screen } from "@testing-library/react";
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

/** 当前 deck 中的 toast 卡片（data-slot=toast），DOM 顺序即 z 序（前 = 最新）。 */
function toastRoots() {
  return Array.from(document.querySelectorAll<HTMLElement>('[data-slot="toast"]'));
}

function viewport() {
  return document.querySelector<HTMLElement>('[data-slot="toast-viewport"]');
}

/** 冲刷入场动画（双 rAF）：推进假计时器并落定 React 状态更新。 */
async function flushEnter() {
  await act(async () => {
    vi.advanceTimersByTime(100);
  });
}

describe("EventBubble（dsh 事件 toast deck）", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-21T10:00:00Z"));
    vi.clearAllMocks();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("窗口失焦（桌宠 nonactivating panel 常态）仍按 8s 自动消失", async () => {
    await renderReady();
    // 复现真机 bug：Base UI 在窗口失焦时暂停自身计时，桌宠面板永不获焦 → 卡片永不过期。
    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    emitSpeak("失焦也要消失");
    act(() => {
      vi.advanceTimersByTime(8500);
    });
    expect(screen.queryByText("失焦也要消失")).toBeNull();
  });

  it("堆叠层级：最新在前可交互，后面各层 pointer-events 关闭并逐层上移微缩", async () => {
    await renderReady();
    emitSpeak("一");
    emitSpeak("二");
    emitSpeak("三");
    await flushEnter();
    const roots = toastRoots();
    expect(roots).toHaveLength(3);
    // 最新（"三"）layer 0：可交互、无上移。
    expect(roots[0].style.pointerEvents).toBe("auto");
    expect(roots[0].style.transform).toContain("translateY(0px)");
    // layer 1：上移 10px、缩至 0.98、不可交互。
    expect(roots[1].style.pointerEvents).toBe("none");
    expect(roots[1].style.transform).toContain("translateY(-10px)");
    expect(roots[1].style.transform).toContain("scale(0.98)");
    // layer 2：上移 20px、缩至 0.96。
    expect(roots[2].style.transform).toContain("translateY(-20px)");
    expect(roots[2].style.transform).toContain("scale(0.96)");
    // 层级递减。
    expect(Number(roots[0].style.zIndex)).toBeGreaterThan(Number(roots[1].style.zIndex));
    expect(Number(roots[1].style.zIndex)).toBeGreaterThan(Number(roots[2].style.zIndex));
  });

  it("新卡入场：挂载帧透明且自下方 14px，下一帧过渡就位", async () => {
    await renderReady();
    emitSpeak("入场动画");
    const card = toastRoots()[0];
    expect(card.style.opacity).toBe("0");
    expect(card.style.transform).toContain("translateY(14px)");
    // rAF 帧冲刷后：就位（透明度 1、回到层位）。
    await flushEnter();
    expect(card.style.opacity).toBe("1");
    expect(card.style.transform).toContain("translateY(0px)");
  });

  it("订阅 dsh-speak，收到事件渲染卡片，8 秒后自动消失", async () => {
    await renderReady();
    expect(listenMock).toHaveBeenCalledWith("dsh-speak", expect.any(Function));
    emitSpeak("任务搞定啦！");
    expect(screen.getByText("任务搞定啦！")).toBeTruthy();
    act(() => {
      vi.advanceTimersByTime(8500);
    });
    expect(screen.queryByText("任务搞定啦！")).toBeNull();
  });

  it("多条消息堆叠成 deck：全部渲染，最新的在最前（index 0）", async () => {
    await renderReady();
    emitSpeak("一");
    emitSpeak("二");
    emitSpeak("三");
    const roots = toastRoots();
    expect(roots).toHaveLength(3);
    // 最新（"三"）在最前：toast-index = 0。
    expect(roots[0].style.getPropertyValue("--toast-index")).toBe("0");
    expect(roots[0].textContent).toContain("三");
    // 折叠态较旧的卡片内容让位（data-behind）。
    expect(roots[2].textContent).toContain("一");
  });

  it("队列上限 3：第 4 条到达时最旧的标记 data-limited 让位", async () => {
    await renderReady();
    emitSpeak("一");
    emitSpeak("二");
    emitSpeak("三");
    emitSpeak("四");
    const roots = toastRoots();
    expect(roots).toHaveLength(4);
    const limited = roots.filter((r) => r.hasAttribute("data-limited"));
    expect(limited).toHaveLength(1);
    expect(limited[0].textContent).toContain("一");
  });

  it("hover 展开、移开收回：卡片 data-expanded 跟随 viewport 悬停", async () => {
    await renderReady();
    emitSpeak("悬停我");
    const vp = viewport();
    expect(vp).toBeTruthy();
    act(() => {
      fireEvent.mouseEnter(vp!);
      fireEvent.mouseMove(vp!);
    });
    expect(toastRoots()[0].hasAttribute("data-expanded")).toBe(true);
    act(() => {
      fireEvent.mouseLeave(vp!);
    });
    expect(toastRoots()[0].hasAttribute("data-expanded")).toBe(false);
  });

  it("悬停暂停计时：hover 中不消失，移开后按剩余时间消失", async () => {
    await renderReady();
    emitSpeak("读我久一点");
    act(() => {
      fireEvent.mouseEnter(viewport()!);
    });
    act(() => {
      vi.advanceTimersByTime(20000);
    });
    expect(screen.getByText("读我久一点")).toBeTruthy();
    act(() => {
      fireEvent.mouseLeave(viewport()!);
    });
    act(() => {
      vi.advanceTimersByTime(8500);
    });
    expect(screen.queryByText("读我久一点")).toBeNull();
  });

  it("卡片上的 mousedown/contextmenu 不冒泡（不触发窗口拖动/右键菜单）", async () => {
    const onMouseDown = vi.fn();
    const onContextMenu = vi.fn();
    render(
      <div onMouseDown={onMouseDown} onContextMenu={onContextMenu}>
        <EventBubble />
      </div>,
    );
    await act(async () => {
      await Promise.resolve();
    });
    emitSpeak("别拖走我");
    const card = toastRoots()[0];
    act(() => {
      fireEvent.mouseDown(card);
      fireEvent.contextMenu(card);
    });
    expect(onMouseDown).not.toHaveBeenCalled();
    expect(onContextMenu).not.toHaveBeenCalled();
  });
});
