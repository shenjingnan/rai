import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useVoiceSession } from "./useVoiceSession";

const { invokeMock, listenMock, eventHandlers } = vi.hoisted(() => {
  const handlers: Record<string, (payload: unknown) => void> = {};
  return {
    invokeMock: vi.fn().mockResolvedValue(undefined),
    listenMock: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
      handlers[event] = (payload) => cb({ payload });
      return Promise.resolve(() => {});
    }),
    eventHandlers: handlers,
  };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

function emit(event: string, payload: unknown) {
  act(() => {
    eventHandlers[event]?.(payload);
  });
}

function Probe() {
  const voice = useVoiceSession();
  return (
    <div>
      <span data-testid="running">{String(voice.running)}</span>
      <span data-testid="phase">{voice.phase}</span>
      <span data-testid="partial">{voice.partial}</span>
      <span data-testid="reply">{voice.replyText}</span>
      <span data-testid="current">{voice.currentSentence ?? ""}</span>
      <span data-testid="segments">{voice.userSegments.map((s) => s.text).join("|")}</span>
      <span data-testid="error">{voice.error ?? ""}</span>
      <button data-testid="start" onClick={() => void voice.start()}>
        start
      </button>
      <button data-testid="stop" onClick={() => void voice.stop()}>
        stop
      </button>
    </div>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  invokeMock.mockResolvedValue(undefined);
});

describe("useVoiceSession", () => {
  it("回读后端运行态并订阅事件驱动状态", async () => {
    invokeMock.mockResolvedValueOnce(true);
    render(<Probe />);
    await waitFor(() => expect(screen.getByTestId("running").textContent).toBe("true"));

    emit("voice-session-state", { running: true, state: "armed" });
    expect(screen.getByTestId("phase").textContent).toBe("armed");

    emit("voice-session-transcript", { text: "你", is_final: false });
    expect(screen.getByTestId("partial").textContent).toBe("你");
    emit("voice-session-transcript", { text: "你好", is_final: true });
    expect(screen.getByTestId("partial").textContent).toBe("");
    expect(screen.getByTestId("segments").textContent).toContain("你好");
  });

  it("LLM token 累积、播放句更新", () => {
    render(<Probe />);
    emit("voice-session-token", { delta: "今天" });
    emit("voice-session-token", { delta: "天气不错。" });
    expect(screen.getByTestId("reply").textContent).toBe("今天天气不错。");

    emit("voice-session-play", { sentence: "今天天气不错。" });
    expect(screen.getByTestId("current").textContent).toBe("今天天气不错。");
  });

  it("stopped 复位为 idle 并透传错误", () => {
    render(<Probe />);
    emit("voice-session-state", { running: true, state: "speaking" });
    emit("voice-session-stopped", { error: "缺模型" });
    expect(screen.getByTestId("running").textContent).toBe("false");
    expect(screen.getByTestId("phase").textContent).toBe("idle");
    expect(screen.getByTestId("error").textContent).toBe("缺模型");
  });

  it("start/stop 调用对应 command", async () => {
    const user = userEvent.setup();
    render(<Probe />);
    await user.click(screen.getByTestId("start"));
    expect(invokeMock).toHaveBeenCalledWith("start_voice_session");

    await user.click(screen.getByTestId("stop"));
    expect(invokeMock).toHaveBeenCalledWith("stop_voice_session");
  });
});
