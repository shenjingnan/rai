import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";

const { invokeMock, listeners, dialogOpenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listeners: new Map<string, (e: { payload: unknown }) => void>(),
  dialogOpenMock: vi.fn(),
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

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: dialogOpenMock,
}));

const KWS_CONFIG = {
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

/** GenParams::default + threads 取已 resolve 值（8 表示物理核数-2）。 */
const DEFAULT_PARAMS = {
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
};

function makeLlmConfig() {
  return {
    enabled: false,
    provider: "local",
    model_path: "/home/user/.zapmomo/models/qwen3-4b.gguf",
    models_present: false,
    ready: false,
    enable_thinking: false,
    auto_load: false,
    settings_path: "/home/user/.zapmomo/settings.toml",
    system_prompt: "你是 ZapMomo 的 AI 大脑。",
    params: { ...DEFAULT_PARAMS },
  };
}

let llmConfig: ReturnType<typeof makeLlmConfig>;

function renderLlmPage() {
  return render(
    <MemoryRouter initialEntries={["/models/llm"]}>
      <App />
    </MemoryRouter>,
  );
}

/** 触发 `llm-status` 事件，把模型置为已加载（模拟后台加载完成 / 启动自动加载）。 */
function fireReady() {
  llmConfig.ready = true; // 同步 mock 状态，使后续 refreshConfig 也读到已就绪（贴近真实后端）
  act(() => {
    listeners.get("llm-status")?.({ payload: { ready: true } });
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  listeners.clear();
  dialogOpenMock.mockReset();
  llmConfig = makeLlmConfig();

  invokeMock.mockImplementation(
    (
      cmd: string,
      args?: {
        enabled?: boolean;
        path?: string;
        params?: Record<string, number>;
        prompt?: string;
      },
    ) => {
      switch (cmd) {
        case "get_app_info":
          return Promise.resolve({ version: "0.1.4", product_name: "ZapMomo" });
        case "list_devices":
          return Promise.resolve(["内置麦克风"]);
        case "get_kws_config":
          return Promise.resolve(KWS_CONFIG);
        case "is_listening":
          return Promise.resolve(false);
        case "get_asr_config":
          return Promise.resolve(ASR_CONFIG);
        case "get_tts_config":
          return Promise.resolve(TTS_CONFIG);
        case "list_tts_voices":
          return Promise.resolve([]);
        case "get_llm_config":
          // 返回新对象引用，保证 refreshConfig 触发重渲染（mock 中 setter 就地改 llmConfig）
          return Promise.resolve({ ...llmConfig });
        case "is_asr_listening":
          return Promise.resolve(false);
        case "is_llm_ready":
          return Promise.resolve(false);
        case "set_llm_auto_load":
          llmConfig.auto_load = args?.enabled ?? false;
          return Promise.resolve(undefined);
        case "set_llm_thinking":
          llmConfig.enable_thinking = args?.enabled ?? false;
          return Promise.resolve(undefined);
        case "set_llm_model_path":
          llmConfig.model_path = args?.path ?? "";
          llmConfig.models_present = true;
          return Promise.resolve(undefined);
        case "set_llm_params":
          // 替换为新对象引用（贴近真实后端：保存后 resolve 出新 params），
          // 使组件里保存前的旧 params 与刷新后的新值可区分（用于判断「哪些字段改了」）
          llmConfig.params = { ...llmConfig.params, ...args?.params };
          return Promise.resolve(undefined);
        case "set_llm_system_prompt":
          llmConfig.system_prompt = args?.prompt ?? "";
          return Promise.resolve(undefined);
        // load/unload/chat/stop 命令直接返回 undefined（由 default 兜底）
        default:
          return Promise.resolve(undefined);
      }
    },
  );
});

describe("LlmPage（AI 大脑配置）", () => {
  it("渲染页面标题与状态摘要", async () => {
    renderLlmPage();
    expect(await screen.findByText("AI 大脑（LLM）配置")).toBeInTheDocument();
    expect(screen.getByText("模型与能力")).toBeInTheDocument();
    expect(screen.getAllByText("当前模型").length).toBeGreaterThan(0);
  });

  it("未配置模型：显示未选择模型/未配置模型，运行开关 disabled，选择模型可用，测试/卸载禁用", async () => {
    renderLlmPage();
    expect((await screen.findAllByText("未选择模型")).length).toBeGreaterThan(0);
    expect(await screen.findByText("未配置模型")).toBeInTheDocument();

    const runSwitch = screen.getByRole("switch", { name: "模型加载开关" });
    expect(runSwitch).toBeDisabled();
    expect(runSwitch).toHaveAttribute("aria-checked", "false");
    expect(screen.getByRole("button", { name: "选择模型" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "测试模型" })).toBeDisabled();
  });

  it("已配置未加载：显示未加载与模型名，运行开关 OFF 可用，测试禁用", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    expect(await screen.findByText("未加载")).toBeInTheDocument();
    expect((await screen.findAllByText("qwen3-4b")).length).toBeGreaterThan(0);

    const runSwitch = screen.getByRole("switch", { name: "模型加载开关" });
    expect(runSwitch).toBeEnabled();
    expect(runSwitch).toHaveAttribute("aria-checked", "false");
    expect(screen.getByRole("button", { name: "测试模型" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "选择模型" })).toBeEnabled();
  });

  it("点击顶部运行开关调用 load_llm_model 并进入加载中（各控件禁用防重复）", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");
    const user = userEvent.setup();

    await user.click(screen.getByRole("switch", { name: "模型加载开关" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("load_llm_model");
    });
    expect(await screen.findByText("加载中")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "模型加载开关" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "选择模型" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "测试模型" })).toBeDisabled();
  });

  it("加载完成（llm-status ready）显示已加载，运行开关 ON，测试与选择模型可用", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");

    fireReady();

    expect(await screen.findByText("已加载")).toBeInTheDocument();
    const runSwitch = screen.getByRole("switch", { name: "模型加载开关" });
    expect(runSwitch).toHaveAttribute("aria-checked", "true");
    expect(runSwitch).toBeEnabled();
    expect(screen.getByRole("button", { name: "测试模型" })).toBeEnabled();
    // 已加载时可选择模型：pick 会静默触发 reload 无缝切换
    expect(screen.getByRole("button", { name: "选择模型" })).toBeEnabled();
  });

  it("已加载时选择模型：set_llm_model_path 后自动 reload 无缝切换", async () => {
    llmConfig.models_present = true;
    dialogOpenMock.mockResolvedValue("/home/user/.zapmomo/models/qwen3-1.7b.gguf");
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "选择模型" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_llm_model_path", {
        path: "/home/user/.zapmomo/models/qwen3-1.7b.gguf",
      });
    });
    // 已加载时 pick 会静默触发 load_llm_model 无缝切换
    expect(invokeMock).toHaveBeenCalledWith("load_llm_model");
    // 新模型名回显
    expect((await screen.findAllByText("qwen3-1.7b")).length).toBeGreaterThan(0);
  });

  it("点击顶部运行开关 OFF 调用 unload_llm_model 并回到未加载", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    await user.click(screen.getByRole("switch", { name: "模型加载开关" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("unload_llm_model");
    });
    expect(await screen.findByText("未加载")).toBeInTheDocument();
  });

  it("切换「启动时自动加载模型」调用 set_llm_auto_load 并持久化开关状态", async () => {
    renderLlmPage();
    const autoLoadSwitch = await screen.findByRole("switch", { name: "启动时自动加载模型" });
    expect(autoLoadSwitch).toHaveAttribute("aria-checked", "false");
    const user = userEvent.setup();

    await user.click(autoLoadSwitch);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_llm_auto_load", { enabled: true });
    });
    await waitFor(() => {
      expect(screen.getByRole("switch", { name: "启动时自动加载模型" })).toHaveAttribute(
        "aria-checked",
        "true",
      );
    });
  });

  it("切换「思考模式」调用 set_llm_thinking 并持久化开关状态", async () => {
    renderLlmPage();
    const thinkingSwitch = await screen.findByRole("switch", { name: "思考模式" });
    expect(thinkingSwitch).toHaveAttribute("aria-checked", "false");
    const user = userEvent.setup();

    await user.click(thinkingSwitch);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_llm_thinking", { enabled: true });
    });
    await waitFor(() => {
      expect(screen.getByRole("switch", { name: "思考模式" })).toHaveAttribute(
        "aria-checked",
        "true",
      );
    });
  });

  it("测试对话框：发送文本、流式接收 token、生成结束后隐藏停止按钮", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "测试模型" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("测试 AI 大脑")).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: "测试消息" }), "你好");
    await user.click(screen.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("chat_llm", { text: "你好" });
    });
    expect(screen.getByRole("button", { name: "停止" })).toBeInTheDocument();

    act(() => {
      listeners.get("llm-token")?.({ payload: { text: "你好世界", is_final: false } });
    });
    expect(await screen.findByText("你好世界")).toBeInTheDocument();

    act(() => {
      listeners.get("llm-finished")?.({ payload: "eos" });
    });
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "停止" })).not.toBeInTheDocument();
    });
  });

  it("生成中点击「停止」调用 stop_llm", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "测试模型" }));
    await user.type(screen.getByRole("textbox", { name: "测试消息" }), "hello");
    await user.click(screen.getByRole("button", { name: "发送" }));
    await screen.findByRole("button", { name: "停止" });

    await user.click(screen.getByRole("button", { name: "停止" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("stop_llm");
    });
  });

  it("关闭测试对话框不卸载模型", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "测试模型" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    await user.keyboard("{Escape}");

    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(invokeMock).not.toHaveBeenCalledWith("unload_llm_model", expect.anything());
  });

  it("选择模型：文件对话框选 GGUF 后调用 set_llm_model_path 并刷新为未加载", async () => {
    dialogOpenMock.mockResolvedValue("/home/user/.zapmomo/models/qwen3-1.7b.gguf");
    renderLlmPage();
    await screen.findByText("未配置模型");
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "选择模型" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_llm_model_path", {
        path: "/home/user/.zapmomo/models/qwen3-1.7b.gguf",
      });
    });
    expect((await screen.findAllByText("qwen3-1.7b")).length).toBeGreaterThan(0);
    expect(await screen.findByText("未加载")).toBeInTheDocument();
  });

  it("高级参数回显解析后的参数值", async () => {
    renderLlmPage();
    const tempInput = await screen.findByRole("textbox", { name: "温度" });
    await waitFor(() => {
      expect(tempInput).toHaveValue("0.7");
    });
    expect(screen.getByRole("textbox", { name: "上下文大小" })).toHaveValue("8192");
    expect(screen.getByRole("textbox", { name: "GPU 层数" })).toHaveValue("0");
  });

  it("修改温度并保存：调用 set_llm_params 且回显新值", async () => {
    renderLlmPage();
    const tempInput = await screen.findByRole("textbox", { name: "温度" });
    await waitFor(() => {
      expect(tempInput).toHaveValue("0.7");
    });
    const user = userEvent.setup();

    await user.clear(tempInput);
    await user.type(tempInput, "0.9");
    await user.click(screen.getByRole("button", { name: "保存参数" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_llm_params",
        expect.objectContaining({ params: expect.objectContaining({ temperature: 0.9 }) }),
      );
    });
    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "温度" })).toHaveValue("0.9");
    });
  });

  it("越界参数保存：不调用 invoke 且显示内联错误", async () => {
    renderLlmPage();
    const ctxInput = await screen.findByRole("textbox", { name: "上下文大小" });
    await waitFor(() => {
      expect(ctxInput).toHaveValue("8192");
    });
    const user = userEvent.setup();

    await user.clear(ctxInput);
    await user.type(ctxInput, "100");
    await user.click(screen.getByRole("button", { name: "保存参数" }));

    expect(invokeMock).not.toHaveBeenCalledWith("set_llm_params", expect.anything());
    expect(await screen.findByText(/上下文大小 需在 256~1048576/)).toBeInTheDocument();
  });

  it("修改系统提示词并保存：调用 set_llm_system_prompt", async () => {
    renderLlmPage();
    const textarea = await screen.findByRole("textbox", { name: "系统提示词" });
    await waitFor(() => {
      expect(textarea).toHaveValue("你是 ZapMomo 的 AI 大脑。");
    });
    const user = userEvent.setup();

    await user.clear(textarea);
    await user.type(textarea, "你是新的提示词。");
    await user.click(screen.getByRole("button", { name: "保存提示词" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_llm_system_prompt", {
        prompt: "你是新的提示词。",
      });
    });
  });

  it("已加载时保存上下文大小：自动 reload 使改动生效", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    const ctxInput = await screen.findByRole("textbox", { name: "上下文大小" });
    await waitFor(() => {
      expect(ctxInput).toHaveValue("8192");
    });
    await user.clear(ctxInput);
    await user.type(ctxInput, "4096");
    await user.click(screen.getByRole("button", { name: "保存参数" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_llm_params",
        expect.objectContaining({ params: expect.objectContaining({ context_size: 4096 }) }),
      );
    });
    expect(invokeMock).toHaveBeenCalledWith("load_llm_model");
  });

  it("已加载时只改温度保存：不触发 reload", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    const tempInput = await screen.findByRole("textbox", { name: "温度" });
    await waitFor(() => {
      expect(tempInput).toHaveValue("0.7");
    });
    await user.clear(tempInput);
    await user.type(tempInput, "0.9");
    await user.click(screen.getByRole("button", { name: "保存参数" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_llm_params",
        expect.objectContaining({ params: expect.objectContaining({ temperature: 0.9 }) }),
      );
    });
    expect(invokeMock).not.toHaveBeenCalledWith("load_llm_model");
  });

  it("已加载时保存系统提示词：自动 reload", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    const textarea = await screen.findByRole("textbox", { name: "系统提示词" });
    await waitFor(() => {
      expect(textarea).toHaveValue("你是 ZapMomo 的 AI 大脑。");
    });
    await user.clear(textarea);
    await user.type(textarea, "新提示词。");
    await user.click(screen.getByRole("button", { name: "保存提示词" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_llm_system_prompt", {
        prompt: "新提示词。",
      });
    });
    expect(invokeMock).toHaveBeenCalledWith("load_llm_model");
  });

  it("参数与提示词未改动时保存按钮 disabled", async () => {
    renderLlmPage();
    const paramsSave = await screen.findByRole("button", { name: "保存参数" });
    await waitFor(() => {
      expect(paramsSave).toBeDisabled();
    });
    expect(screen.getByRole("button", { name: "保存提示词" })).toBeDisabled();
  });

  it("模型路径默认隐藏，点击图标展开/收起", async () => {
    llmConfig.models_present = true;
    renderLlmPage();
    await screen.findByText("未加载");

    const pathText = "/home/user/.zapmomo/models/qwen3-4b.gguf";
    expect(screen.queryByText(pathText)).not.toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "查看模型路径" }));
    expect(screen.getByText(pathText)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "隐藏模型路径" }));
    expect(screen.queryByText(pathText)).not.toBeInTheDocument();
  });
});
