import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";
import type { LibraryModel } from "@/types/modelLibrary";

const { invokeMock, listeners, dialogOpenMock, llmDownloadImpl } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listeners: new Map<string, (e: { payload: unknown }) => void>(),
  dialogOpenMock: vi.fn(),
  /** `download_llm_model` 的可注入实现（各用例控制挂起/成功/失败/applied） */
  llmDownloadImpl: vi.fn(),
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
/** `list_model_library` 返回的已注册模型（含 LLM 预设的安装/current 状态）。 */
let libraryModels: LibraryModel[];
/** set_current_model 可替换实现（beforeEach 重置；loading 用例覆盖为挂起 promise）。 */
let switchImpl: (id: string) => Promise<unknown>;

/** 已安装的 managed LLM 预设（LlmPresetDialog 按 id/repoId 匹配）。 */
function installedPreset(
  id: string,
  name: string,
  opts?: { current?: boolean; installState?: LibraryModel["installState"] },
): LibraryModel {
  return {
    id,
    name,
    displayName: name,
    modelType: "llm",
    runtime: "llama.cpp",
    format: "GGUF",
    description: "",
    languages: [],
    tags: [],
    parameterCount: "0.6B",
    quantization: "Q4_K_M",
    version: "instruct",
    sizeBytes: 396_705_472,
    homepage: null,
    downloadable: true,
    source: "registry",
    ownership: "managed",
    installState: opts?.installState ?? "installed",
    current: opts?.current ?? false,
    runtimeStatus: "inactive",
    localPath: `/home/user/.zapmomo/models/${name}`,
    installedAt: "2026-08-20T00:00:00Z",
    installId: id,
    repoId: null,
    compatibility: "verified",
  };
}

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
  llmDownloadImpl.mockReset();
  llmDownloadImpl.mockResolvedValue({
    model_path: "/home/user/.zapmomo/models/Qwen3-0.6B/Qwen3-0.6B-Q4_K_M.gguf",
    applied: true,
  });
  // set_current_model 默认实现（loading 用例可覆盖为挂起 promise）。
  switchImpl = (id: string) => {
    libraryModels = libraryModels.map((m) => ({ ...m, current: m.id === id }));
    const target = libraryModels.find((m) => m.id === id);
    if (target) {
      llmConfig.model_path = target.localPath ?? "";
      llmConfig.models_present = true;
    }
    return Promise.resolve({
      model_type: "llm",
      model_id: id,
      path: target?.localPath ?? "",
      runtime_action: "none",
      effective_immediately: false,
      message: "已设为当前模型",
    });
  };
  llmConfig = makeLlmConfig();
  libraryModels = [];

  invokeMock.mockImplementation(
    (
      cmd: string,
      args?: {
        enabled?: boolean;
        path?: string;
        params?: Record<string, number>;
        prompt?: string;
        id?: string;
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
        case "is_voice_session_running":
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
        case "download_llm_model":
          // 模拟后端：applied=true 时写入配置（下载区据此消失、开关解锁）
          return llmDownloadImpl(args).then((r: { model_path: string; applied: boolean }) => {
            if (r.applied) {
              llmConfig.model_path = r.model_path;
              llmConfig.models_present = true;
            }
            return r;
          });
        case "list_model_library":
          return Promise.resolve(libraryModels);
        case "set_current_model":
          return switchImpl(args?.id as string);
        case "delete_model": {
          const id = args?.id as string;
          libraryModels = libraryModels.filter((m) => m.id !== id);
          return Promise.resolve(undefined);
        }
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

  it("已加载时选择模型：弹窗内导入 GGUF 后 set_llm_model_path 自动 reload 无缝切换", async () => {
    llmConfig.models_present = true;
    dialogOpenMock.mockResolvedValue("/home/user/.zapmomo/models/qwen3-1.7b.gguf");
    renderLlmPage();
    await screen.findByText("未加载");
    fireReady();
    await screen.findByText("已加载");
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "选择模型" }));
    await user.click(await screen.findByRole("button", { name: "导入 GGUF 文件" }));

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

  it("选择模型：弹窗内导入 GGUF 文件调 set_llm_model_path 并刷新为未加载", async () => {
    dialogOpenMock.mockResolvedValue("/home/user/.zapmomo/models/qwen3-1.7b.gguf");
    renderLlmPage();
    await screen.findByText("未配置模型");
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "选择模型" }));
    await user.click(await screen.findByRole("button", { name: "导入 GGUF 文件" }));

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

  describe("选择模型弹窗（内置预设模型）", () => {
    /** 渲染 LLM 页并打开「选择模型」弹窗，返回 userEvent。 */
    async function renderAndOpenPresets() {
      renderLlmPage();
      const user = userEvent.setup();
      await user.click(await screen.findByRole("button", { name: "选择模型" }));
      await screen.findByRole("dialog");
      return user;
    }

    it("弹窗显示 5 个推荐预设与下载按钮、导入入口、模型库链接", async () => {
      await renderAndOpenPresets();
      expect(screen.getByText("Qwen3 0.6B")).toBeInTheDocument();
      expect(screen.getByText("Qwen3 1.7B")).toBeInTheDocument();
      expect(screen.getByText("Qwen3 4B")).toBeInTheDocument();
      expect(screen.getByText("Qwen3 8B")).toBeInTheDocument();
      expect(screen.getByText("Llama 3.2 1B")).toBeInTheDocument();
      expect(screen.getByText(/378\.3 MB · 约 1GB 内存/)).toBeInTheDocument();
      expect(screen.getByText(/2\.33 GB · 约 4GB 内存/)).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "下载Qwen3 0.6B" })).toBeEnabled();
      expect(screen.getByRole("button", { name: "导入 GGUF 文件" })).toBeInTheDocument();
      expect(screen.getByRole("link", { name: /更多模型/ })).toBeInTheDocument();
    });

    it("点击预设调用 download_llm_model，下载中其他预设禁用", async () => {
      llmDownloadImpl.mockReturnValue(new Promise(() => {})); // 挂起模拟下载中
      const user = await renderAndOpenPresets();
      await user.click(screen.getByRole("button", { name: "下载Qwen3 0.6B" }));

      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("download_llm_model", {
          id: "qwen3-0.6b-q4-k-m",
        });
      });
      expect(screen.getByRole("button", { name: "下载Qwen3 0.6B" })).toBeDisabled();
      expect(screen.getByRole("button", { name: "下载Qwen3 4B" })).toBeDisabled();
      expect(screen.getByText("下载中…")).toBeInTheDocument();
    });

    it("下载进度事件：显示百分比消息", async () => {
      llmDownloadImpl.mockReturnValue(new Promise(() => {}));
      const user = await renderAndOpenPresets();
      await user.click(screen.getByRole("button", { name: "下载Qwen3 0.6B" }));
      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("download_llm_model", expect.anything());
      });

      act(() => {
        listeners.get("llm-model-download-progress")?.({
          payload: { stage: "downloading", percent: 42, message: "下载中 42.0%" },
        });
      });
      expect(await screen.findByText("下载中 42.0%")).toBeInTheDocument();
    });

    it("下载完成（applied）：自动配置并加载", async () => {
      const user = await renderAndOpenPresets();
      await user.click(screen.getByRole("button", { name: "下载Qwen3 0.6B" }));

      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("download_llm_model", {
          id: "qwen3-0.6b-q4-k-m",
        });
      });
      expect(await screen.findByText("加载中")).toBeInTheDocument();
      expect(invokeMock).toHaveBeenCalledWith("load_llm_model");
    });

    it("voice 会话运行中下载完成：不自动加载", async () => {
      renderLlmPage();
      // 先等 mount 时的 isVoiceSessionRunning 回读落定，否则其异步 setRunning 会覆盖事件状态
      await Promise.resolve();
      act(() => {
        listeners.get("voice-session-state")?.({ payload: { running: true, state: "armed" } });
      });
      const user = userEvent.setup();
      await user.click(await screen.findByRole("button", { name: "选择模型" }));
      await user.click(await screen.findByRole("button", { name: "下载Qwen3 0.6B" }));

      // 等 refreshConfig 完成（get_llm_config 第 2 次调用），load 判断已同步执行
      await waitFor(() => {
        expect(invokeMock.mock.calls.filter(([c]) => c === "get_llm_config").length).toBe(2);
      });
      expect(invokeMock).not.toHaveBeenCalledWith("load_llm_model");
    });

    it("下载失败：显示错误并恢复按钮", async () => {
      llmDownloadImpl.mockRejectedValue("下载失败：网络错误");
      const user = await renderAndOpenPresets();
      await user.click(screen.getByRole("button", { name: "下载Qwen3 0.6B" }));

      expect(await screen.findByText(/下载失败：网络错误/)).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "下载Qwen3 0.6B" })).toBeEnabled();
    });

    it("远程 provider（openai）选择模型按钮禁用，弹窗不渲染", async () => {
      llmConfig.provider = "openai";
      renderLlmPage();
      expect(await screen.findByText("AI 大脑（LLM）配置")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "选择模型" })).toBeDisabled();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("已配置模型时选择模型按钮可用，弹窗可打开（用于切换）", async () => {
      llmConfig.models_present = true;
      await renderAndOpenPresets();
      expect(screen.getByRole("dialog")).toBeInTheDocument();
    });

    it("applied=false（下载期间用户已自行配置）：不覆盖配置、不自动加载", async () => {
      llmDownloadImpl.mockResolvedValue({
        model_path: "/home/user/.zapmomo/models/Qwen3-0.6B/Qwen3-0.6B-Q4_K_M.gguf",
        applied: false,
      });
      const user = await renderAndOpenPresets();
      await user.click(screen.getByRole("button", { name: "下载Qwen3 0.6B" }));

      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("download_llm_model", expect.anything());
      });
      // 等 refreshConfig 完成（get_llm_config 第 2 次调用），load 判断已同步执行
      await waitFor(() => {
        expect(invokeMock.mock.calls.filter(([c]) => c === "get_llm_config").length).toBe(2);
      });
      expect(screen.getByRole("button", { name: "下载Qwen3 0.6B" })).toBeInTheDocument();
      expect(invokeMock).not.toHaveBeenCalledWith("load_llm_model");
    });

    it("已安装非当前预设：显示设为当前与卸载；设为当前调 set_current_model", async () => {
      libraryModels = [installedPreset("qwen3-0.6b-q4-k-m", "Qwen3 0.6B")];
      const user = await renderAndOpenPresets();
      await waitFor(() => {
        expect(screen.queryByRole("button", { name: "下载Qwen3 0.6B" })).not.toBeInTheDocument();
      });
      expect(screen.getByRole("button", { name: "设为当前" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "卸载" })).toBeInTheDocument();
      await user.click(screen.getByRole("button", { name: "设为当前" }));
      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("set_current_model", {
          id: "qwen3-0.6b-q4-k-m",
        });
      });
    });

    it("设为当前等待后端事务期间：按钮转圈禁用（热切换数秒需 loading 反馈）", async () => {
      libraryModels = [installedPreset("qwen3-0.6b-q4-k-m", "Qwen3 0.6B")];
      // set_current_model 挂起（模拟后端卸旧载新数秒），直到手动 resolve。
      const gate = { release: null as null | (() => void) };
      switchImpl = () =>
        new Promise((resolve) => {
          gate.release = () =>
            resolve({
              modelType: "llm",
              modelId: "qwen3-0.6b-q4-k-m",
              path: "/models/qwen3-0.6b.gguf",
              runtimeAction: "reloaded",
              effectiveImmediately: true,
              message: "已设为当前模型",
            });
        });
      const user = await renderAndOpenPresets();
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "设为当前" })).toBeInTheDocument();
      });

      await user.click(screen.getByRole("button", { name: "设为当前" }));
      const busy = await screen.findByRole("button", { name: "切换中…" });
      expect(busy).toBeDisabled();
      expect(screen.getByRole("button", { name: "卸载" })).toBeDisabled();

      gate.release?.();
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "设为当前" })).toBeEnabled();
      });
    });

    it("未安装（not_installed 记录）预设显示下载，而非设为当前/卸载", async () => {
      // 回归：list_model_library 对未安装 registry 模型也返回记录，仅按 id 匹配会误判为已安装
      libraryModels = [
        installedPreset("qwen3-0.6b-q4-k-m", "Qwen3 0.6B", { installState: "not_installed" }),
      ];
      await renderAndOpenPresets();
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "下载Qwen3 0.6B" })).toBeInTheDocument();
      });
      expect(screen.queryByRole("button", { name: "设为当前" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "卸载" })).not.toBeInTheDocument();
    });

    it("当前预设显示当前模型标记，无设为当前/卸载", async () => {
      libraryModels = [installedPreset("qwen3-0.6b-q4-k-m", "Qwen3 0.6B", { current: true })];
      await renderAndOpenPresets();
      // 「当前模型」同时出现在弹窗预设行标记与 LlmCoreConfig 的字段标签，取列表形式断言
      await waitFor(() => {
        expect(screen.getAllByText("当前模型").length).toBeGreaterThan(0);
      });
      expect(screen.queryByRole("button", { name: "设为当前" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "卸载" })).not.toBeInTheDocument();
    });

    it("卸载预设：确认框 → 确认 → delete_model", async () => {
      libraryModels = [installedPreset("qwen3-0.6b-q4-k-m", "Qwen3 0.6B")];
      const user = await renderAndOpenPresets();
      await waitFor(() => {
        expect(screen.queryByRole("button", { name: "下载Qwen3 0.6B" })).not.toBeInTheDocument();
      });
      await user.click(screen.getByRole("button", { name: "卸载" }));
      expect(await screen.findByText(/确定要卸载/)).toBeInTheDocument();
      await user.click(screen.getByRole("button", { name: "确认卸载" }));
      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("delete_model", { id: "qwen3-0.6b-q4-k-m" });
      });
    });
  });
});
