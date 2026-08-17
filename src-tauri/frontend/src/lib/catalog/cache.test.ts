import { describe, expect, it, vi } from "vitest";
import { TtlCache } from "./cache";

describe("TtlCache", () => {
  it("命中与过期", () => {
    vi.useFakeTimers();
    const c = new TtlCache<number>(1000);
    c.set("a", 1);
    expect(c.get("a")).toBe(1);
    vi.advanceTimersByTime(1001);
    expect(c.get("a")).toBeNull();
    vi.useRealTimers();
  });

  it("超过容量 FIFO 淘汰", () => {
    const c = new TtlCache<number>(5000, 2);
    c.set("a", 1);
    c.set("b", 2);
    c.set("c", 3);
    expect(c.get("a")).toBeNull(); // 最早淘汰
    expect(c.get("b")).toBe(2);
    expect(c.get("c")).toBe(3);
  });

  it("clear 清空", () => {
    const c = new TtlCache<number>(5000);
    c.set("a", 1);
    c.clear();
    expect(c.get("a")).toBeNull();
  });
});
