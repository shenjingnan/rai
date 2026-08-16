import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const { invokeMock, listeners } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listeners: new Map<string, (e: { payload: unknown }) => void>(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => {});
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  })),
}));

const DEFAULT_CONFIG = {
  model_dir: "/home/user/.zapmomo/models/sherpa-onnx-kws",
  provider: "cpu",
  num_threads: 4,
  sample_rate: 16000,
  keywords: ["文森特卡索"],
  models_present: false,
  model_downloading: false,
  settings_path: "/home/user/.zapmomo/settings.toml",
};

/** 渲染 App 并定位到 KWS 详情页（模型相关 UI 所在页面）。 */
function renderApp() {
  return render(
    <MemoryRouter initialEntries={["/models/kws"]}>
      <App />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
  listeners.clear();

  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_app_info":
        return Promise.resolve({ version: "0.1.4", product_name: "ZapMomo" });
      case "list_devices":
        return Promise.resolve(["内置麦克风", "USB 麦克风"]);
      case "get_kws_config":
        return Promise.resolve(DEFAULT_CONFIG);
      case "is_listening":
        return Promise.resolve(false);
      case "start_listen":
      case "stop_listen":
      case "download_kws_model":
        return Promise.resolve(undefined);
      default:
        return Promise.resolve(undefined);
    }
  });
});

describe("App（KWS 控制面板）", () => {
  it("渲染 Sidebar 导航与空闲状态", async () => {
    renderApp();
    expect(screen.getByAltText("ZapMomo")).toBeInTheDocument();
    expect(screen.getByText("首页")).toBeInTheDocument();
    expect(await screen.findByText("空闲")).toBeInTheDocument();
  });

  it("渲染 KWS 配置项", async () => {
    renderApp();
    expect(
      await screen.findByText("/home/user/.zapmomo/models/sherpa-onnx-kws"),
    ).toBeInTheDocument();
    expect(screen.getByText("cpu / 4")).toBeInTheDocument();
    expect(screen.getByText("16000")).toBeInTheDocument();
    expect(screen.getByText("文森特卡索")).toBeInTheDocument();
    expect(screen.getByText("/home/user/.zapmomo/settings.toml")).toBeInTheDocument();
  });

  it("模型缺失时显示警告与下载按钮", async () => {
    renderApp();
    expect(await screen.findByText("模型文件缺失")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /下载模型/ })).toBeInTheDocument();
  });

  it("点击开始监听调用 start_listen 并进入监听中状态", async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByText("空闲");

    await user.click(screen.getByRole("button", { name: /开始监听/ }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_listen", {
        device: null,
        keywords: null,
      });
    });
    expect(await screen.findByText("监听中")).toBeInTheDocument();
  });

  it("点击停止监听调用 stop_listen", async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByText("空闲");

    await user.click(screen.getByRole("button", { name: /开始监听/ }));
    await screen.findByText("监听中");

    await user.click(screen.getByRole("button", { name: /停止监听/ }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("stop_listen");
    });
  });

  it("检测到唤醒词后把结果追加到列表", async () => {
    renderApp();
    await screen.findByText("尚未检测到唤醒词");

    act(() => {
      listeners.get("kws-detected")?.({
        payload: {
          keyword: "文森特卡索",
          tokens: "",
          tokens_arr: [],
          timestamps: [],
          start_time: 0.64,
          json: "{}",
        },
      });
    });

    expect(await screen.findByText(/start=0\.64s/)).toBeInTheDocument();
  });

  it("点击下载模型调用 download_kws_model 并刷新配置", async () => {
    const user = userEvent.setup();
    renderApp();
    const button = await screen.findByRole("button", { name: /下载模型/ });

    await user.click(button);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("download_kws_model");
    });
    // 下载完成后会重新拉取配置（models_present 变 true 后按钮消失）
    await waitFor(() => {
      const calls = invokeMock.mock.calls.map((c) => c[0]);
      expect(calls.filter((c) => c === "get_kws_config").length).toBeGreaterThanOrEqual(2);
    });
  });
});
