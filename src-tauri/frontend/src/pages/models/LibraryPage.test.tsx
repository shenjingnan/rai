import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";
import type { LibraryModel } from "@/types/modelLibrary";

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

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));

const BASE: Omit<LibraryModel, "id" | "name" | "displayName" | "modelType"> = {
  runtime: "sherpa-onnx",
  format: "ONNX",
  description: "测试模型",
  languages: ["zh", "en"],
  tags: [],
  parameterCount: null,
  quantization: null,
  version: "1.0",
  sizeBytes: 1024,
  homepage: null,
  downloadable: false,
  source: "registry",
  ownership: "managed",
  installState: "not_installed",
  current: false,
  runtimeStatus: "inactive",
  localPath: null,
  installedAt: null,
};

const MODELS: LibraryModel[] = [
  {
    ...BASE,
    id: "qwen3-1.7b-q4-k-m",
    name: "Qwen3-1.7B",
    displayName: "Qwen3 1.7B Q4_K_M",
    modelType: "llm",
    runtime: "llama.cpp",
    format: "GGUF",
    tags: ["qwen3", "thinking"],
  },
  {
    ...BASE,
    id: "kws-zipformer-zh-en-3m",
    name: "sherpa-onnx-kws-zipformer-zh-en-3M",
    displayName: "唤醒词模型（Zipformer）",
    modelType: "kws",
    downloadable: true,
    tags: ["streaming", "lightweight"],
  },
];

let models: LibraryModel[];

function defaultInvoke(cmd: string) {
  switch (cmd) {
    case "get_app_info":
      return Promise.resolve({ version: "0.1.4", product_name: "ZapMomo" });
    case "list_devices":
      return Promise.resolve(["内置麦克风"]);
    case "get_kws_config":
      return Promise.resolve({ model_dir: "", models_present: false, model_downloading: false });
    case "get_asr_config":
      return Promise.resolve({ model_dir: "", models_present: false, model_downloading: false });
    case "get_tts_config":
      return Promise.resolve({
        model_dir: "",
        models_present: false,
        model_downloading: false,
        enabled: true,
      });
    case "get_llm_config":
      return Promise.resolve({
        model_path: "",
        models_present: false,
        ready: false,
        loaded_model_path: null,
        enabled: false,
        auto_load: false,
        enable_thinking: false,
      });
    case "list_model_library":
      return Promise.resolve(models);
    case "is_listening":
    case "is_asr_listening":
    case "is_tts_synthesizing":
    case "is_llm_ready":
      return Promise.resolve(false);
    case "list_tts_voices":
      return Promise.resolve([]);
    case "get_live2d_config":
      return Promise.resolve({ model_dir: "", models_present: false });
    case "get_microphone":
      return Promise.resolve("");
    case "get_system_resources":
      return Promise.resolve({
        totalMemory: 16 * 1024 ** 3,
        availableMemory: 8 * 1024 ** 3,
        diskTotal: 500 * 1024 ** 3,
        diskAvailable: 200 * 1024 ** 3,
        cpuUsage: 12,
      });
    default:
      return Promise.resolve(undefined);
  }
}

beforeEach(() => {
  models = MODELS.map((m) => ({ ...m }));
  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string) => defaultInvoke(cmd));
});

describe("LibraryPage", () => {
  it("渲染模型列表：KWS 可下载、LLM 需导入", async () => {
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "模型库" })).toBeInTheDocument();
    });
    expect(await screen.findByText("Qwen3 1.7B Q4_K_M")).toBeInTheDocument();
    expect(screen.getByText("唤醒词模型（Zipformer）")).toBeInTheDocument();
    // LLM（无内置下载源）→ 导入 GGUF；KWS（可下载）→ 下载
    expect(screen.getByRole("button", { name: "导入 GGUF" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "下载" })).toBeInTheDocument();
  });

  it("类型 Tab 只显示对应类型", async () => {
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    await screen.findByText("Qwen3 1.7B Q4_K_M");
    await userEvent.click(screen.getByRole("button", { name: "LLM" }));
    expect(screen.getByText("Qwen3 1.7B Q4_K_M")).toBeInTheDocument();
    expect(screen.queryByText("唤醒词模型（Zipformer）")).not.toBeInTheDocument();
  });

  it("搜索实时过滤名称/描述/标签", async () => {
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    await screen.findByText("Qwen3 1.7B Q4_K_M");
    await userEvent.type(screen.getByPlaceholderText("搜索模型名称、描述或标签..."), "Qwen");
    expect(screen.getByText("Qwen3 1.7B Q4_K_M")).toBeInTheDocument();
    expect(screen.queryByText("唤醒词模型（Zipformer）")).not.toBeInTheDocument();
  });

  it("空结果显示空状态", async () => {
    models = [];
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByText("没有找到符合条件的模型")).toBeInTheDocument();
  });
});
