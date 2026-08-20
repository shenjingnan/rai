import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LlmState } from "@/hooks/useLlm";
import { LlmModelSwitchMenu } from "./LlmModelSwitchMenu";

// stub 选择模型弹窗：只记录 props，避免其内部（toast/invoke/plugin-dialog）整链依赖。
const { dialogProps } = vi.hoisted(() => ({
  dialogProps: { last: null as { open: boolean; onClose: () => void } | null },
}));

vi.mock("@/components/llm/LlmPresetDialog", () => ({
  LlmPresetDialog: (props: { open: boolean; onClose: () => void }) => {
    dialogProps.last = props;
    return props.open ? <div data-testid="preset-dialog">选择模型弹窗</div> : null;
  },
}));

// mock runtime：llm 切片可变（provider / model_path）。
const { state } = vi.hoisted(() => ({ state: { llm: null as LlmState | null } }));

vi.mock("@/providers/RuntimeContext", () => ({
  useRuntime: () => ({ llm: state.llm }),
}));

function makeLlm(o?: { modelPath?: string; provider?: string }): LlmState {
  return {
    config: {
      models_present: true,
      model_path: o?.modelPath ?? "/models/qwen3-4b.gguf",
      provider: o?.provider ?? "local",
    },
    configError: null,
    error: null,
    ready: false,
    loading: false,
    refreshConfig: vi.fn(),
    load: vi.fn(),
    unload: vi.fn(),
  } as unknown as LlmState;
}

/** 挂真实链接行验证「不触发导航」；location 探针放在 /models。 */
function Probe() {
  const location = useLocation();
  return (
    <>
      <div data-testid="location">{location.pathname}</div>
      <a href="/models/llm" data-testid="row-link" onClick={(e) => e.preventDefault()}>
        <LlmModelSwitchMenu />
      </a>
    </>
  );
}

function renderMenu() {
  return render(
    <MemoryRouter initialEntries={["/models"]}>
      <Routes>
        <Route path="/models" element={<Probe />} />
        <Route path="/models/llm" element={<div>配置页</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  dialogProps.last = null;
  state.llm = makeLlm();
});

describe("LlmModelSwitchMenu 模型快速切换（弹窗版）", () => {
  it("模型名文本 + 明显的切换按钮", () => {
    renderMenu();
    expect(screen.getByText("qwen3-4b.gguf")).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "切换 AI 大脑模型" });
    expect(button).toHaveTextContent("切换");
  });

  it("点击切换按钮打开选择模型弹窗", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "切换 AI 大脑模型" }));

    expect(dialogProps.last?.open).toBe(true);
    expect(screen.getByTestId("preset-dialog")).toBeInTheDocument();
  });

  it("弹窗 onClose 回调关闭后可再次打开", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "切换 AI 大脑模型" }));
    expect(dialogProps.last?.open).toBe(true);

    // onClose 触发 setState，等待 stub 以 open=false 重渲染。
    act(() => dialogProps.last?.onClose());
    await waitFor(() => expect(dialogProps.last?.open).toBe(false));

    await user.click(screen.getByRole("button", { name: "切换 AI 大脑模型" }));
    await waitFor(() => expect(dialogProps.last?.open).toBe(true));
  });

  it("点击行内按钮/弹窗不触发所在行的链接导航", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "切换 AI 大脑模型" }));
    expect(screen.getByTestId("preset-dialog")).toBeInTheDocument();
    expect(screen.getByTestId("location")).toHaveTextContent("/models");
  });

  it("HTTP API 模式：只显示模型名，无切换按钮与弹窗", () => {
    state.llm = makeLlm({ provider: "openai" });
    renderMenu();
    expect(screen.getByText("qwen3-4b.gguf")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "切换 AI 大脑模型" })).not.toBeInTheDocument();
  });
});
