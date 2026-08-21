/**
 * 键名归一化：把模拟器发出的 rdev Debug 键名映射到模型实际拥有贴图的键名。
 *
 * 移植自 BongoCat `useDevice.getSupportedKey` 的语义：模型没有该键贴图时，
 * 功能键家族（`F1~F12`）退化为 `Fn`、修饰键家族（`Meta*`/`Shift*`/`Alt*`/`Control*`）
 * 退化为基础名（`Meta`/`Shift`/`Alt`/`Control`）；仍不可映射返回 `null`（忽略该键）。
 */

/** 修饰键家族前缀（按 BongoCat 顺序）。 */
const MODIFIER_PREFIXES = ["Meta", "Shift", "Alt", "Control"] as const;

/**
 * 返回归一化后的键名；不可映射返回 `null`。
 *
 * @param key 模拟器事件里的键名（如 `KeyA`、`ShiftLeft`、`F1`）
 * @param supportKeys 模型实际拥有贴图的键名集合
 */
export function getSupportedKey(key: string, supportKeys: ReadonlySet<string>): string | null {
  if (supportKeys.has(key)) {
    return key;
  }
  // 功能键家族 → Fn（仅当模型有 Fn 贴图）
  if (/^F\d+$/.test(key) && supportKeys.has("Fn")) {
    return "Fn";
  }
  // 修饰键家族 → 基础名（仅当模型有基础名贴图）
  for (const base of MODIFIER_PREFIXES) {
    if (key.startsWith(base) && supportKeys.has(base)) {
      return base;
    }
  }
  return null;
}
