import { describe, expect, it } from "vitest";
import { Damper } from "./damper";

describe("Damper", () => {
  it("无目标时 update 返回 null", () => {
    const d = new Damper();
    expect(d.update(16.7)).toBeNull();
  });

  it("首次 setTarget 以目标为起点（不突跳）", () => {
    const d = new Damper();
    d.setTarget({ x: 100, y: 50 });
    expect(d.value).toEqual({ x: 100, y: 50 });
  });

  it("alpha 公式数值断言（deltaMS=16.7 → 0.25）", () => {
    const alpha = 1 - 0.75 ** (16.7 / (1000 / 60));
    expect(alpha).toBeCloseTo(0.25, 2);
  });

  it("固定步进下逐步逼近目标并收敛", () => {
    const d = new Damper();
    d.setTarget({ x: 0, y: 0 });
    d.setTarget({ x: 100, y: 0 }); // 起点已为 (0,0)
    let steps = 0;
    let lastDist = Infinity;
    // 步进直到收敛（settled），上限 200 步防死循环
    while (!d.settled && steps < 200) {
      const pos = d.update(16.7);
      if (!pos) throw new Error("未收敛时 update 不应返回 null");
      const dist = Math.hypot(100 - pos.x, 0 - pos.y);
      expect(dist).toBeLessThan(lastDist); // 距离严格递减
      lastDist = dist;
      steps += 1;
    }
    expect(steps).toBeLessThan(200);
    expect(d.settled).toBe(true);
    expect(d.value).toEqual({ x: 100, y: 0 });
  });

  it("reset 后回到无目标状态", () => {
    const d = new Damper();
    d.setTarget({ x: 10, y: 10 });
    d.update(16.7);
    d.reset();
    expect(d.value).toBeNull();
    expect(d.update(16.7)).toBeNull();
  });
});
