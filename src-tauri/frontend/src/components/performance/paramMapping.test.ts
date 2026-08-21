import { describe, expect, it } from "vitest";
import { handParams, mapCursorToParams, mouseButtonParams } from "./paramMapping";

function rangeOfFactory(full: Record<string, { min: number; max: number }>) {
  return (id: string) => full[id] ?? null;
}

const FULL_RANGES = {
  ParamMouseX: { min: -30, max: 30 },
  ParamMouseY: { min: -30, max: 30 },
  ParamAngleX: { min: -30, max: 30 },
  ParamAngleY: { min: -30, max: 30 },
  ParamAngleZ: { min: -30, max: 30 },
  ParamEyeBallX: { min: -1, max: 1 },
  ParamEyeBallY: { min: -1, max: 1 },
};

describe("mapCursorToParams", () => {
  it("包含 7 个跟随参数", () => {
    const result = mapCursorToParams(0.5, 0.5, rangeOfFactory(FULL_RANGES));
    expect(result).toHaveLength(7);
    expect(result.map((m) => m.id)).toEqual(Object.keys(FULL_RANGES));
  });

  it("X 轴 xRatio=0 → max，xRatio=1 → min", () => {
    const rangeOf = rangeOfFactory(FULL_RANGES);
    const at0 = mapCursorToParams(0, 0.5, rangeOf).find((m) => m.id === "ParamAngleX");
    const at1 = mapCursorToParams(1, 0.5, rangeOf).find((m) => m.id === "ParamAngleX");
    expect(at0?.value).toBe(30);
    expect(at1?.value).toBe(-30);
  });

  it("Y 轴 yRatio=0 → max（屏幕顶部 → 参数正方向）", () => {
    const rangeOf = rangeOfFactory(FULL_RANGES);
    const at0 = mapCursorToParams(0.5, 0, rangeOf).find((m) => m.id === "ParamMouseY");
    const at1 = mapCursorToParams(0.5, 1, rangeOf).find((m) => m.id === "ParamMouseY");
    expect(at0?.value).toBe(30);
    expect(at1?.value).toBe(-30);
  });

  it("Z 轴用 dragX*dragY*min", () => {
    const rangeOf = rangeOfFactory(FULL_RANGES);
    // 左上角：dragX=1, dragY=1 → 1*1*min = -30
    const topLeft = mapCursorToParams(0, 0, rangeOf).find((m) => m.id === "ParamAngleZ");
    expect(topLeft?.value).toBe(-30);
    // 中心：dragX=0 → 0
    const center = mapCursorToParams(0.5, 0.5, rangeOf).find((m) => m.id === "ParamAngleZ");
    expect(center?.value).toBeCloseTo(0, 5);
  });

  it("缺失参数被跳过（防御性降级）", () => {
    const onlyHead = rangeOfFactory({ ParamAngleX: { min: -30, max: 30 } });
    const result = mapCursorToParams(0.5, 0.5, onlyHead);
    expect(result).toEqual([{ id: "ParamAngleX", value: 0 }]);
  });
});

describe("handParams", () => {
  it("手按下 → 1，松开 → 0", () => {
    expect(handParams(true, false)).toEqual([
      { id: "CatParamLeftHandDown", value: 1 },
      { id: "CatParamRightHandDown", value: 0 },
    ]);
    expect(handParams(false, true)).toEqual([
      { id: "CatParamLeftHandDown", value: 0 },
      { id: "CatParamRightHandDown", value: 1 },
    ]);
  });
});

describe("mouseButtonParams", () => {
  it("鼠标键按下 → 1，松开 → 0", () => {
    expect(mouseButtonParams(true, false)).toEqual([
      { id: "ParamMouseLeftDown", value: 1 },
      { id: "ParamMouseRightDown", value: 0 },
    ]);
    expect(mouseButtonParams(false, false)).toEqual([
      { id: "ParamMouseLeftDown", value: 0 },
      { id: "ParamMouseRightDown", value: 0 },
    ]);
  });
});
