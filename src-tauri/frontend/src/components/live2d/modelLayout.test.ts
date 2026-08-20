import { describe, expect, it, vi } from "vitest";
import { computeModelBounds, layoutModel } from "./modelLayout";

/**
 * 假造一个最小 Cubism4 内部模型：originalWidth/Height=100×200、局部宽高=200×400
 * （layout 缩放因子 sx=sy=2），两个 drawable 合并包围盒。
 */
function makeModel(rects: Record<string, { x: number; y: number; width: number; height: number }>) {
  return {
    internalModel: {
      width: 200,
      height: 400,
      originalWidth: 100,
      originalHeight: 200,
      getDrawableIDs: () => Object.keys(rects),
      getDrawableIndex: (id: string) => Object.keys(rects).indexOf(id),
      getDrawableBounds: (index: number) => rects[Object.keys(rects)[index]],
    },
    scale: { set: vi.fn() },
    anchor: { set: vi.fn() },
    position: { set: vi.fn() },
  };
}

describe("computeModelBounds", () => {
  it("合并所有 drawable 边界并按 layout 缩放因子映射到模型局部坐标", () => {
    // a: (10,20)-(40,60)，b: (50,60)-(70,80) → 原始空间合并 (10,20)-(70,80)，×2 → 120×120
    const model = makeModel({
      a: { x: 10, y: 20, width: 30, height: 40 },
      b: { x: 50, y: 60, width: 20, height: 20 },
    });

    expect(computeModelBounds(model as never)).toEqual({
      cx: 80, // (10+70)/2*2
      cy: 100, // (20+80)/2*2
      width: 120, // (70-10)*2
      height: 120, // (80-20)*2
    });
  });

  it("无 drawable 时返回非法包围盒（NaN/Infinity），供上层跳过布局", () => {
    const model = makeModel({});
    const bounds = computeModelBounds(model as never);
    expect(Number.isFinite(bounds.width)).toBe(false);
    expect(Number.isFinite(bounds.height)).toBe(false);
  });
});

describe("layoutModel", () => {
  it("按包围盒 contain 撑满画布并居中：scale=min(w/bw,h/bh)，position 居中偏移", () => {
    const model = makeModel({
      a: { x: 10, y: 20, width: 30, height: 40 },
      b: { x: 50, y: 60, width: 20, height: 20 },
    });
    // bounds=120×120，画布 240×240 → scale=2；modelScale=0.8 → 1.6
    layoutModel(model as never, 240, 240, 0.8);

    expect(model.scale.set).toHaveBeenCalledWith(1.6);
    expect(model.anchor.set).toHaveBeenCalledWith(0, 0);
    // x = 240/2 - 80*1.6 = -8，y = 240/2 - 100*1.6 = -40
    expect(model.position.set).toHaveBeenCalledWith(-8, -40);
  });

  it("不传 modelScale 默认 1（完整 contain 填充）", () => {
    const model = makeModel({ a: { x: 0, y: 0, width: 100, height: 200 } });
    // bounds=200×400，画布 200×400 → scale=min(1,1)=1
    layoutModel(model as never, 200, 400);

    expect(model.scale.set).toHaveBeenCalledWith(1);
  });

  it("包围盒非法（空 drawable 等）时跳过布局，保持默认状态", () => {
    const model = makeModel({});
    layoutModel(model as never, 240, 240);

    expect(model.scale.set).not.toHaveBeenCalled();
    expect(model.position.set).not.toHaveBeenCalled();
  });
});
