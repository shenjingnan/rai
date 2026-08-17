import { describe, expect, it, vi } from "vitest";
import {
  buildCatalogQuery,
  categoryToPipelineTag,
  compatibilityLabel,
  compatibilityRank,
  debounce,
  isInstallable,
} from "./query";

describe("buildCatalogQuery", () => {
  it("映射 UI 状态 → CatalogQuery", () => {
    const q = buildCatalogQuery(
      {
        category: "llm",
        search: "  qwen  ",
        language: "zh",
        license: "apache-2.0",
        parameters: "b3_to_7",
        sort: "downloads",
      },
      2,
    );
    expect(q.category).toBe("llm");
    expect(q.search).toBe("qwen"); // trim
    expect(q.parameters).toBe("b3_to_7");
    expect(q.sort).toBe("downloads");
    expect(q.page).toBe(2);
    expect(q.pageSize).toBe(20);
  });

  it("空搜索 → null", () => {
    const q = buildCatalogQuery(
      {
        category: null,
        search: "   ",
        language: null,
        license: null,
        parameters: null,
        sort: "recommended",
      },
      0,
    );
    expect(q.search).toBeNull();
  });
});

describe("categoryToPipelineTag", () => {
  it("映射分类 → pipeline tag", () => {
    expect(categoryToPipelineTag("llm")).toBe("text-generation");
    expect(categoryToPipelineTag("asr")).toBe("automatic-speech-recognition");
    expect(categoryToPipelineTag("tts")).toBe("text-to-speech");
    expect(categoryToPipelineTag("kws")).toBe("wake-word");
  });
});

describe("compatibility", () => {
  it("仅 Verified/Compatible 可安装", () => {
    expect(isInstallable("verified")).toBe(true);
    expect(isInstallable("compatible")).toBe(true);
    expect(isInstallable("possible")).toBe(false);
    expect(isInstallable("unsupported")).toBe(false);
  });

  it("等级排序权重", () => {
    expect(compatibilityRank("verified")).toBeGreaterThan(compatibilityRank("compatible"));
    expect(compatibilityRank("possible")).toBeGreaterThan(compatibilityRank("unsupported"));
  });

  it("中文文案", () => {
    expect(compatibilityLabel("verified")).toBe("已验证");
    expect(compatibilityLabel("possible")).toBe("待确认兼容");
  });
});

describe("debounce", () => {
  it("快速多次调用只执行最后一次", async () => {
    vi.useFakeTimers();
    const fn = vi.fn();
    const d = debounce(fn, 100);
    d.run(1);
    d.run(2);
    d.run(3);
    expect(fn).not.toHaveBeenCalled();
    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith(3);
    vi.useRealTimers();
  });

  it("cancel 取消执行", async () => {
    vi.useFakeTimers();
    const fn = vi.fn();
    const d = debounce(fn, 100);
    d.run(1);
    d.cancel();
    vi.advanceTimersByTime(200);
    expect(fn).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});
