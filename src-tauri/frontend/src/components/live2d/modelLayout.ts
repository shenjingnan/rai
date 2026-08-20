import type { Live2DModel } from "pixi-live2d-display/cubism4";

/** 角色真实包围盒（模型局部坐标），用于居中 + 等比缩放。 */
export interface ModelBounds {
  cx: number;
  cy: number;
  width: number;
  height: number;
}

/**
 * 遍历所有 drawable，合并边界得到角色真实最小包围盒（AABB）。
 *
 * `getDrawableBounds` 返回原始画布空间（originalWidth×originalHeight），
 * 乘以 layout 缩放因子（internalModel.width / originalWidth）映射到模型局部坐标。
 */
export function computeModelBounds(model: Live2DModel): ModelBounds {
  const im = model.internalModel;
  const sx = im.width / im.originalWidth;
  const sy = im.height / im.originalHeight;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const id of im.getDrawableIDs()) {
    const b = im.getDrawableBounds(im.getDrawableIndex(id));
    minX = Math.min(minX, b.x);
    minY = Math.min(minY, b.y);
    maxX = Math.max(maxX, b.x + b.width);
    maxY = Math.max(maxY, b.y + b.height);
  }
  return {
    cx: ((minX + maxX) / 2) * sx,
    cy: ((minY + maxY) / 2) * sy,
    width: (maxX - minX) * sx,
    height: (maxY - minY) * sy,
  };
}

/**
 * 让角色真实包围盒在画布内 contain 撑满并居中（而非基于画布尺寸）。
 * `modelScale` 额外乘一个等比系数（<1 缩小），用于概览等场景让模型小一圈。
 * 若包围盒非法（空 drawable 等），跳过布局，保持模型默认状态。
 */
export function layoutModel(model: Live2DModel, width: number, height: number, modelScale = 1) {
  const b = computeModelBounds(model);
  if (!Number.isFinite(b.width) || !Number.isFinite(b.height) || b.width <= 0 || b.height <= 0) {
    return;
  }
  const scale = Math.min(width / b.width, height / b.height) * modelScale;
  model.scale.set(scale);
  model.anchor.set(0, 0);
  model.position.set(width / 2 - b.cx * scale, height / 2 - b.cy * scale);
}
