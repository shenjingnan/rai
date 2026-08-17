import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";
import { queryClient } from "@/lib/queryClient";
import type { CatalogPage, UnifiedModelItem } from "@/types/catalog";

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

function unifiedItem(
  modelId: string,
  compatibility: UnifiedModelItem["compatibility"] = "compatible",
): UnifiedModelItem {
  return {
    canonicalKey: `huggingface:${modelId.toLowerCase()}`,
    modelId,
    provider: "huggingface",
    remote: {
      repoId: modelId,
      author: "Qwen",
      displayName: modelId.split("/")[1] ?? modelId,
      description: `测试描述 ${modelId}`,
      pipelineTag: "text-generation",
      libraryName: "gguf",
      tags: ["qwen3"],
      downloads: 1000,
      likes: 50,
      trendingScore: null,
      lastModified: "2025-05-20T00:00:00Z",
      createdAt: null,
      license: "apache-2.0",
      languages: ["zh"],
      parameterCount: "4B",
      gated: null,
      private: null,
      sha: null,
    },
    builtin: null,
    modelType: "llm",
    compatibility,
    compatibilityNotes: null,
    recommendedVariant: null,
    installs: [],
    localSummary: { installedArtifactCount: 0, hasCurrentArtifact: false, activeDownloadCount: 0 },
    confirmed: false,
  };
}

let catalogPage: CatalogPage<UnifiedModelItem>;

function defaultInvoke(cmd: string, args?: Record<string, unknown>) {
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
      return Promise.resolve([]);
    case "catalog_search_models": {
      // 支持 category / search 的简单过滤（模拟 HF 服务端）
      const query = args?.query as { category?: string; search?: string };
      let items = catalogPage.items;
      if (query?.category) items = items.filter((i) => i.modelType === query.category);
      if (query?.search) {
        const q = query.search.toLowerCase();
        items = items.filter(
          (i) =>
            i.modelId.toLowerCase().includes(q) ||
            (i.remote?.description ?? "").toLowerCase().includes(q),
        );
      }
      return Promise.resolve({ items, hasMore: false });
    }
    case "catalog_get_model_detail":
      return Promise.resolve({
        repoId: "",
        description: null,
        pipelineTag: null,
        libraryName: null,
        tags: [],
        license: null,
        languages: [],
        downloads: 0,
        likes: 0,
        lastModified: null,
        createdAt: null,
        sha: null,
        gated: null,
        private: null,
        cardData: null,
        siblings: [],
      });
    case "download_snapshot":
      return Promise.resolve([]);
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
    default:
      return Promise.resolve(undefined);
  }
}

beforeEach(() => {
  catalogPage = {
    items: [unifiedItem("Qwen/Qwen3-4B-GGUF"), unifiedItem("Qwen/Qwen3-0.6B-GGUF")],
    hasMore: false,
  };
  queryClient.clear(); // 隔离 React Query 缓存（单例跨测试共享）
  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string, args?: Record<string, unknown>) =>
    defaultInvoke(cmd, args),
  );
});

describe("LibraryPage", () => {
  it("渲染在线目录（HF 真实数据形态）", async () => {
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    await waitFor(
      () => {
        expect(screen.getByText("Qwen3-4B-GGUF")).toBeInTheDocument();
      },
      { timeout: 3000 },
    );
    expect(screen.getByText("Qwen3-0.6B-GGUF")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "添加本地模型" })).toBeInTheDocument();
    expect(screen.getByText("Hugging Face")).toBeInTheDocument();
  });

  it("搜索触发远程查询（debounce 后）", async () => {
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("Qwen3-4B-GGUF")).toBeInTheDocument(), {
      timeout: 3000,
    });
    await userEvent.type(screen.getByPlaceholderText("搜索模型名称、描述、标签或作者..."), "0.6B");
    // 等 debounce 后的远程查询：0.6B 出现且 4B 消失（服务端过滤）
    await waitFor(
      () => {
        expect(screen.getByText("Qwen3-0.6B-GGUF")).toBeInTheDocument();
        expect(screen.queryByText("Qwen3-4B-GGUF")).not.toBeInTheDocument();
      },
      { timeout: 3000 },
    );
  });

  it("分类 Tab 存在且切换不崩溃", async () => {
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("Qwen3-4B-GGUF")).toBeInTheDocument(), {
      timeout: 3000,
    });
    const llmTab = screen.getByRole("button", { name: "LLM" });
    expect(llmTab).toBeInTheDocument();
    await userEvent.click(llmTab);
    expect(screen.getByText("Qwen3-4B-GGUF")).toBeInTheDocument();
  });

  it("空结果显示空状态", async () => {
    catalogPage = { items: [], hasMore: false };
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    await waitFor(
      () => {
        expect(screen.getByText("没有找到符合条件的模型")).toBeInTheDocument();
      },
      { timeout: 3000 },
    );
  });

  it("默认只显示可用模型；打开「显示全部模型」后展示可能兼容/不兼容", async () => {
    catalogPage = {
      items: [
        unifiedItem("Qwen/Qwen3-4B-GGUF", "compatible"),
        unifiedItem("Some/Transformers", "possible"),
        unifiedItem("Some/Whisper", "unsupported"),
      ],
      hasMore: false,
    };
    render(
      <MemoryRouter initialEntries={["/models/library"]}>
        <App />
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("Qwen3-4B-GGUF")).toBeInTheDocument(), {
      timeout: 3000,
    });
    // 默认：只显示可用（compatible），possible / unsupported 项隐藏
    expect(screen.queryByText("Transformers")).not.toBeInTheDocument();
    expect(screen.queryByText("Whisper")).not.toBeInTheDocument();
    // 打开「显示全部模型」→ 所有兼容级别出现
    await userEvent.click(screen.getByText("显示全部模型"));
    await waitFor(() => expect(screen.getByText("Transformers")).toBeInTheDocument(), {
      timeout: 3000,
    });
    expect(screen.getByText("Whisper")).toBeInTheDocument();
  });
});
