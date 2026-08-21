import { describe, expect, it } from "vitest";
import { getSupportedKey } from "./keyNormalize";

describe("getSupportedKey", () => {
  const fullKeys = new Set([
    "KeyA",
    "KeyB",
    "ShiftLeft",
    "Shift",
    "MetaLeft",
    "ControlLeft",
    "Fn",
    "F1",
  ]);

  it("支持键原样返回", () => {
    expect(getSupportedKey("KeyA", fullKeys)).toBe("KeyA");
    expect(getSupportedKey("ShiftLeft", fullKeys)).toBe("ShiftLeft");
    expect(getSupportedKey("F1", fullKeys)).toBe("F1");
  });

  it("F 键在模型无对应贴图时退化为 Fn", () => {
    const keys = new Set(["Fn", "KeyA"]);
    expect(getSupportedKey("F5", keys)).toBe("Fn");
  });

  it("模型有 F1 贴图时 F1 不退化", () => {
    expect(getSupportedKey("F1", fullKeys)).toBe("F1");
  });

  it("修饰键家族退化为基础名", () => {
    const keys = new Set(["Shift", "Meta", "Control", "Alt"]);
    expect(getSupportedKey("ShiftLeft", keys)).toBe("Shift");
    expect(getSupportedKey("MetaRight", keys)).toBe("Meta");
    expect(getSupportedKey("ControlLeft", keys)).toBe("Control");
    expect(getSupportedKey("AltLeft", keys)).toBe("Alt");
  });

  it("不可映射返回 null", () => {
    const keys = new Set(["KeyA"]);
    expect(getSupportedKey("F5", keys)).toBeNull(); // 无 Fn
    expect(getSupportedKey("ShiftLeft", keys)).toBeNull(); // 无 Shift 基础名
    expect(getSupportedKey("CapsLock", keys)).toBeNull();
  });
});
