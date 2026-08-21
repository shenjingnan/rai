/**
 * Live2D 表演参数映射：把模拟光标/按键状态换算成 Cubism 参数目标值。
 *
 * 移植自 BongoCat `useModel.handleMouseMove` / `handleKeyChange` / `handleMouseChange`
 * 的映射规则：
 * - 光标 → 7 个跟随参数（鼠标道具 + 头/眼球），X/Y 轴线性映射、Z 轴 `dragX*dragY*min`、
 *   Y 轴天然反向；
 * - 手按下 → `CatParamLeft/RightHandDown`（0 抬起 / 1 按下）；
 * - 鼠标键按下 → `ParamMouseLeft/RightDown`。
 *
 * 参数是否存在由 `rangeOf` 判断（缺失参数跳过），与 `Live2dParamWriter.range` 协作，
 * 构成「任意模型防御性降级」的第一道防线。
 */

/** 单个参数目标值。 */
export interface ParamMapping {
  id: string;
  value: number;
}

/** 光标跟随参数表（BongoCat 同款，含鼠标道具 + 头部/眼球角度）。 */
const CURSOR_PARAMS = [
  "ParamMouseX",
  "ParamMouseY",
  "ParamAngleX",
  "ParamAngleY",
  "ParamAngleZ",
  "ParamEyeBallX",
  "ParamEyeBallY",
] as const;

/**
 * 把归一化光标坐标（`xRatio/yRatio ∈ [0,1]`，全局物理坐标相对播放区域的比值）
 * 映射为光标跟随参数目标值。缺失参数跳过（`rangeOf` 返回 null）。
 */
export function mapCursorToParams(
  xRatio: number,
  yRatio: number,
  rangeOf: (id: string) => { min: number; max: number } | null,
): ParamMapping[] {
  const out: ParamMapping[] = [];
  for (const id of CURSOR_PARAMS) {
    const range = rangeOf(id);
    if (!range) {
      continue;
    }
    const { min, max } = range;
    const ratio = id.endsWith("X") ? xRatio : yRatio;
    let value: number;
    if (id.endsWith("Z")) {
      const dragX = 1 - 2 * xRatio;
      const dragY = 1 - 2 * yRatio;
      value = dragX * dragY * min;
    } else {
      // X/Y 轴同一式（BongoCat 同款）。Y 轴的"反向"天然由 min/max 范围体现
      // （Y 参数范围通常 min>max，如 30→-30）。
      value = max - ratio * (max - min);
    }
    out.push({ id, value });
  }
  return out;
}

/**
 * 手臂按下参数：手按着 → 1，松开 → 0。参数是否存在交由写入层 `writer.set` 判断
 * （无此参数的普通模型 no-op 无害）。
 */
export function handParams(hasLeft: boolean, hasRight: boolean): ParamMapping[] {
  return [
    { id: "CatParamLeftHandDown", value: hasLeft ? 1 : 0 },
    { id: "CatParamRightHandDown", value: hasRight ? 1 : 0 },
  ];
}

/** 鼠标键按下参数（0 抬起 / 1 按下）。 */
export function mouseButtonParams(leftDown: boolean, rightDown: boolean): ParamMapping[] {
  return [
    { id: "ParamMouseLeftDown", value: leftDown ? 1 : 0 },
    { id: "ParamMouseRightDown", value: rightDown ? 1 : 0 },
  ];
}
