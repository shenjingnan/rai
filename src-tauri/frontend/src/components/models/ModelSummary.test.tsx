import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeState } from "@/providers/RuntimeContext";
import { ModelSummary } from "./ModelSummary";

// 模型摘要读取全局 runtime；直接 mock context 模块，避免挂载完整 App / mock tauri invoke。
const { state } = vi.hoisted(() => ({
  // 可变 runtime 快照：单个用例按需替换 kws/asr/llm/tts 切片（贴近真实 RuntimeState）。
  state: { runtime: null as RuntimeState | null },
}));

vi.mock("@/providers/RuntimeContext", () => ({
  useRuntime: () => state.runtime,
}));

// ---- runtime 切片工厂：只填 ModelSummary 读取的字段（其余方法 vi.fn() 兜底）----

function makeKws(o?: {
  enabled?: boolean;
  modelsPresent?: boolean;
  isListening?: boolean;
  error?: string | null;
}) {
  return {
    config: {
      config: {
        enabled: o?.enabled ?? false,
        models_present: o?.modelsPresent ?? false,
        model_dir: "/zap/.zapmomo/models/kws",
      },
      refresh: vi.fn(),
      error: null,
    },
    listening: {
      isListening: o?.isListening ?? false,
      pending: false,
      error: o?.error ?? null,
    },
  };
}

function makeAsr(o?: {
  enabled?: boolean;
  modelsPresent?: boolean;
  isListening?: boolean;
  pending?: boolean;
  error?: string | null;
}) {
  return {
    config: {
      config: {
        enabled: o?.enabled ?? false,
        models_present: o?.modelsPresent ?? false,
        model_dir: "/zap/.zapmomo/models/asr",
      },
      refresh: vi.fn(),
      error: null,
    },
    listening: {
      isListening: o?.isListening ?? false,
      pending: o?.pending ?? false,
      error: o?.error ?? null,
    },
  };
}

function makeLlm(o?: {
  modelsPresent?: boolean;
  ready?: boolean;
  loading?: boolean;
  error?: string | null;
}) {
  return {
    config: {
      models_present: o?.modelsPresent ?? false,
      model_path: "/zap/.zapmomo/models/llm.gguf",
    },
    ready: o?.ready ?? false,
    loading: o?.loading ?? false,
    error: o?.error ?? null,
    refreshConfig: vi.fn(),
  };
}

function makeTts(o?: { modelsPresent?: boolean; enabled?: boolean; synthesizing?: boolean }) {
  return {
    config: {
      enabled: o?.enabled ?? true,
      models_present: o?.modelsPresent ?? false,
      model_dir: "/zap/.zapmomo/models/tts",
    },
    configError: null,
    synthesizing: o?.synthesizing ?? false,
    refreshConfig: vi.fn(),
  };
}

function makeRuntime(
  overrides?: Partial<{
    kws: ReturnType<typeof makeKws>;
    asr: ReturnType<typeof makeAsr>;
    llm: ReturnType<typeof makeLlm>;
    tts: ReturnType<typeof makeTts>;
  }>,
): RuntimeState {
  return {
    kws: overrides?.kws ?? makeKws(),
    asr: overrides?.asr ?? makeAsr(),
    llm: overrides?.llm ?? makeLlm(),
    tts: overrides?.tts ?? makeTts(),
  } as unknown as RuntimeState;
}

/** 各摘要行的 aria-label（SummaryRow 用 aria-label=`配置${row.name}`）。 */
const ROW_NAME = {
  kws: "配置唤醒词（KWS）",
  asr: "配置语音识别（ASR）",
  llm: "配置AI 大脑（LLM）",
  tts: "配置语音合成（TTS）",
} as const;

function rowFor(key: keyof typeof ROW_NAME): HTMLElement {
  return screen.getByRole("link", { name: ROW_NAME[key] });
}

/** 断言某行显示指定状态文本（可选断言状态色 class，映射 STATUS_COLOR）。 */
function expectRowStatus(row: HTMLElement, text: string, toneClass?: string) {
  // 未配置时模型名 `<p>` 与状态 `<span>` 都会显示「未配置模型」，用 selector 精确定位状态 span。
  const status = within(row).getByText(text, { selector: "span" });
  if (toneClass) expect(status).toHaveClass(toneClass);
}

function renderSummary() {
  return render(
    <MemoryRouter>
      <ModelSummary />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  state.runtime = makeRuntime();
});

describe("ModelSummary 模型摘要状态", () => {
  it("KWS：监听出错 → 错误", () => {
    state.runtime = makeRuntime({
      kws: makeKws({ modelsPresent: true, enabled: true, error: "engine boom" }),
    });
    renderSummary();
    expectRowStatus(rowFor("kws"), "错误", "text-red-600");
  });

  it("KWS：正在监听 → 监听中", () => {
    state.runtime = makeRuntime({
      kws: makeKws({ modelsPresent: true, enabled: true, isListening: true }),
    });
    renderSummary();
    expectRowStatus(rowFor("kws"), "监听中");
  });

  it("KWS：enabled 且模型在但未监听 → 已就绪（读取持久化 enabled）", () => {
    state.runtime = makeRuntime({ kws: makeKws({ modelsPresent: true, enabled: true }) });
    renderSummary();
    expectRowStatus(rowFor("kws"), "已就绪", "text-emerald-600");
  });

  it("KWS：模型在但未启用 → 未启用", () => {
    state.runtime = makeRuntime({ kws: makeKws({ modelsPresent: true, enabled: false }) });
    renderSummary();
    expectRowStatus(rowFor("kws"), "未启用", "text-text-muted");
  });

  it("KWS：无模型 → 未配置模型", () => {
    renderSummary();
    expectRowStatus(rowFor("kws"), "未配置模型");
  });

  it("ASR：监听出错 → 错误", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: true, enabled: true, error: "engine boom" }),
    });
    renderSummary();
    expectRowStatus(rowFor("asr"), "错误", "text-red-600");
  });

  it("ASR：启动中 → 启动中", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: true, enabled: true, pending: true }),
    });
    renderSummary();
    expectRowStatus(rowFor("asr"), "启动中", "text-blue-600");
  });

  it("ASR：正在识别 → 识别中", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: true, enabled: true, isListening: true }),
    });
    renderSummary();
    expectRowStatus(rowFor("asr"), "识别中");
  });

  it("ASR：enabled 且模型在但未识别 → 已就绪（回归：此前误显示未启用）", () => {
    state.runtime = makeRuntime({ asr: makeAsr({ modelsPresent: true, enabled: true }) });
    renderSummary();
    expectRowStatus(rowFor("asr"), "已就绪", "text-emerald-600");
  });

  it("ASR：模型在但未启用 → 未启用（回归：此前误显示已就绪）", () => {
    state.runtime = makeRuntime({ asr: makeAsr({ modelsPresent: true, enabled: false }) });
    renderSummary();
    expectRowStatus(rowFor("asr"), "未启用", "text-text-muted");
  });

  it("ASR：无模型 → 未配置模型", () => {
    renderSummary();
    expectRowStatus(rowFor("asr"), "未配置模型");
  });

  it("LLM 行不受影响：模型在但未加载 → 未启用；KWS/ASR 已就绪互不干扰", () => {
    state.runtime = makeRuntime({
      kws: makeKws({ modelsPresent: true, enabled: true }),
      asr: makeAsr({ modelsPresent: true, enabled: true }),
      llm: makeLlm({ modelsPresent: true }),
    });
    renderSummary();
    expectRowStatus(rowFor("llm"), "未启用");
    expectRowStatus(rowFor("kws"), "已就绪");
    expectRowStatus(rowFor("asr"), "已就绪");
  });

  it("TTS 行不受影响：主动关闭 → 已关闭", () => {
    state.runtime = makeRuntime({ tts: makeTts({ modelsPresent: true, enabled: false }) });
    renderSummary();
    expectRowStatus(rowFor("tts"), "已关闭");
  });

  it("默认全未配置：四行状态均显示未配置模型", () => {
    renderSummary();
    // 每行模型名 `<p>` 也是「未配置模型」，这里只统计状态 span（4 行各 1 个）。
    expect(screen.getAllByText("未配置模型", { selector: "span" })).toHaveLength(4);
  });
});
