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

const ASR_CONFIG = {
  model_dir: "/home/user/.zapmomo/models/sherpa-onnx-streaming-zipformer",
  provider: "cpu",
  num_threads: 4,
  sample_rate: 16000,
  models_present: false,
  punctuation_present: false,
  model_downloading: false,
  settings_path: "/home/user/.zapmomo/settings.toml",
};

const TTS_CONFIG = {
  model_dir: "/home/user/.zapmomo/models/sherpa-onnx-zipvoice",
  provider: "cpu",
  num_threads: 4,
  enabled: true,
  models_present: false,
  model_downloading: false,
  settings_path: "/home/user/.zapmomo/settings.toml",
};

const LLM_CONFIG = {
  enabled: false,
  provider: "local",
  model_path: "/home/user/.zapmomo/models/qwen3-4b.gguf",
  models_present: false,
  ready: false,
  enable_thinking: false,
  auto_load: false,
  settings_path: "/home/user/.zapmomo/settings.toml",
  system_prompt: "你是 ZapMomo 的 AI 大脑。",
  params: {
    context_size: 8192,
    batch_size: 512,
    max_tokens: 512,
    temperature: 0.7,
    top_p: 0.8,
    top_k: 20,
    min_p: 0.05,
    repeat_penalty: 1.05,
    seed: 0,
    threads: 8,
    gpu_layers: 0,
    enable_thinking: false,
  },
};

/** 渲染 App 并定位到指定路由（默认 KWS 详情页）。 */
function renderApp(initialPath = "/models/kws") {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
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
      case "get_asr_config":
        return Promise.resolve(ASR_CONFIG);
      case "get_tts_config":
        return Promise.resolve(TTS_CONFIG);
      case "list_tts_voices":
        return Promise.resolve([]);
      case "get_llm_config":
        return Promise.resolve(LLM_CONFIG);
      case "is_asr_listening":
        return Promise.resolve(false);
      case "is_llm_ready":
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
  it("渲染 Sidebar 导航与模型概览页（能力链路 + 模型摘要）", async () => {
    renderApp("/models");
    expect(screen.getByAltText("ZapMomo")).toBeInTheDocument();
    expect(screen.getByText("概览")).toBeInTheDocument();
    expect(await screen.findByText("AI 能力链路")).toBeInTheDocument();
    expect(screen.getByText("模型摘要")).toBeInTheDocument();
    expect(screen.getByText("管理模型")).toBeInTheDocument();
  });

  it("概览页 ASR 开关调用 start_asr_listen", async () => {
    const user = userEvent.setup();
    renderApp("/models");

    await user.click(await screen.findByRole("switch", { name: "语音输入开关" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_asr_listen", { device: null });
    });
  });

  it("ASR 未开启时点击唤醒词开关弹确认框，确认后同时开启 ASR 与 KWS", async () => {
    const user = userEvent.setup();
    renderApp("/models");

    const kwsSwitch = await screen.findByRole("switch", { name: "唤醒词开关" });
    expect(kwsSwitch).not.toBeDisabled();

    await user.click(kwsSwitch);

    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("需要先开启语音输入")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "同时开启" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_asr_listen", { device: null });
      expect(invokeMock).toHaveBeenCalledWith("start_listen", { device: null, keywords: null });
    });
  });

  it("ASR 未开启时取消确认框则不开启任何能力", async () => {
    const user = userEvent.setup();
    renderApp("/models");

    await user.click(await screen.findByRole("switch", { name: "唤醒词开关" }));
    await user.click(screen.getByRole("button", { name: "取消" }));

    // 退出动画结束后对话框卸载
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(invokeMock).not.toHaveBeenCalledWith("start_asr_listen", expect.anything());
    expect(invokeMock).not.toHaveBeenCalledWith("start_listen", expect.anything());
  });

  it("概览页语音合成开关调用 set_tts_enabled", async () => {
    const user = userEvent.setup();
    renderApp("/models");

    await user.click(await screen.findByRole("switch", { name: "语音合成开关" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_tts_enabled", { enabled: false });
    });
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

    await user.click(await screen.findByRole("button", { name: /开始监听/ }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_listen", {
        device: null,
        keywords: null,
      });
    });
    // 进入监听中状态：停止监听按钮从禁用变为可用
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /停止监听/ })).toBeEnabled();
    });
  });

  it("点击停止监听调用 stop_listen", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(await screen.findByRole("button", { name: /开始监听/ }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /停止监听/ })).toBeEnabled();
    });

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

  it("设置页可切换是否隐藏 Dock / Cmd+Tab 图标", async () => {
    const user = userEvent.setup();
    renderApp("/settings");

    const checkbox = await screen.findByRole("checkbox", { name: /隐藏应用图标/ });
    expect(checkbox).not.toBeChecked();

    await user.click(checkbox);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_hide_dock_icon", { hide: true });
    });
  });
});
