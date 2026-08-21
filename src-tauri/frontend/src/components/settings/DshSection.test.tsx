import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DshSection } from "./DshSection";

const { invokeMock, listenMock, toastMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn((_event: string, _cb: (e: { payload: unknown }) => void) =>
    Promise.resolve(() => {}),
  ),
  // 稳定的 toast 对象：`useToast()` 每次渲染返回同一引用，避免依赖 [toast]
  // 的 useCallback 变化导致 effect 反复执行。
  toastMock: { success: vi.fn(), error: vi.fn() },
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@/components/ui/toast", () => ({
  useToast: () => toastMock,
}));

const baseInfo = {
  enabled: true,
  port: 0,
  voice_enabled: true,
  record_to_history: true,
  running: true,
  actual_port: 52341,
  discovery_path: "/tmp/dsh-bridge.json",
};

describe("DshSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockReset();
  });

  it("载入配置并显示运行端口", async () => {
    invokeMock.mockResolvedValueOnce(baseInfo);
    render(<DshSection />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_dsh_config"));
    expect(screen.getByText(/52341/)).toBeTruthy();
  });

  it("总开关调用 set_dsh_enabled", async () => {
    invokeMock.mockResolvedValue(baseInfo);
    render(<DshSection />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    const toggle = screen.getByRole("switch", { name: "启用 dsh 桥" });
    await userEvent.click(toggle);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_dsh_enabled", { enabled: false }),
    );
  });

  it("测试播报按钮调用 test_dsh_announce", async () => {
    invokeMock.mockResolvedValue(baseInfo);
    render(<DshSection />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    await userEvent.click(screen.getByRole("button", { name: /测试播报/ }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("test_dsh_announce"));
  });
});
