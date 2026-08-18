import * as PIXI from "pixi.js";
import { Live2DModel } from "pixi-live2d-display/cubism4";
import { useEffect, useRef } from "react";

// pixi-live2d-display 依赖全局 window.PIXI.Ticker 驱动渲染循环。
if (typeof window !== "undefined") {
  (window as unknown as { PIXI: typeof PIXI }).PIXI = PIXI;
}

interface Live2dStageProps {
  /** 模型清单文件的 asset:// URL，null 时不加载。 */
  modelUrl: string | null;
  width: number;
  height: number;
  className?: string;
  /** 模型在画布内的等比缩放（<1 缩小、>1 放大，默认 1 = 完整 contain 填充）。 */
  modelScale?: number;
  /** 渲染初始化或模型加载失败时的回调。 */
  onError?: (error: Error) => void;
  /** 模型加载完成、可计算角色真实边界时回调（供上层自适应窗口尺寸）。 */
  onModelMetrics?: (metrics: { aspectRatio: number }) => void;
  /** 模型加载成功、画布已可用时回调（供上层截取封面等；注意画布可能尚未渲染本帧）。 */
  onModelReady?: (canvas: HTMLCanvasElement) => void;
}

/** 角色真实包围盒（模型局部坐标），用于居中 + 等比缩放。 */
interface ModelBounds {
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
function computeModelBounds(model: Live2DModel): ModelBounds {
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
function layoutModel(model: Live2DModel, width: number, height: number, modelScale = 1) {
  const b = computeModelBounds(model);
  if (!Number.isFinite(b.width) || !Number.isFinite(b.height) || b.width <= 0 || b.height <= 0) {
    return;
  }
  const scale = Math.min(width / b.width, height / b.height) * modelScale;
  model.scale.set(scale);
  model.anchor.set(0, 0);
  model.position.set(width / 2 - b.cx * scale, height / 2 - b.cy * scale);
}

/**
 * Live2D 渲染组件：命令式创建 PIXI Application（PIXI 8 同步构造），
 * 规避 React StrictMode 双挂载时 PIXI 移除 DOM 节点导致引用失效的问题。
 *
 * 尺寸变化只 resize 渲染器并重新布局，不销毁重建、不重载模型。
 */
export function Live2dStage({
  modelUrl,
  width,
  height,
  className,
  modelScale = 1,
  onError,
  onModelMetrics,
  onModelReady,
}: Live2dStageProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<PIXI.Application | null>(null);
  const modelRef = useRef<Live2DModel | null>(null);

  // 用 ref 保存最新回调与尺寸，供异步加载流程读取，避免闭包过期。
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;
  const onModelMetricsRef = useRef(onModelMetrics);
  onModelMetricsRef.current = onModelMetrics;
  const onModelReadyRef = useRef(onModelReady);
  onModelReadyRef.current = onModelReady;
  const sizeRef = useRef({ width, height });
  sizeRef.current = { width, height };
  // 模型加载 effect 用 ref 读缩放，避免 scale 变化触发销毁重载；布局由 resize effect 在 deps 里重算。
  const modelScaleRef = useRef(modelScale);
  modelScaleRef.current = modelScale;

  // 创建 / 销毁 PIXI 应用（仅随组件挂载/卸载，不随尺寸变化）。
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let app: PIXI.Application | null = null;
    try {
      app = new PIXI.Application({
        width: sizeRef.current.width,
        height: sizeRef.current.height,
        backgroundAlpha: 0,
        antialias: true,
        autoStart: true,
        // 高分屏（Retina）下按 devicePixelRatio 渲染，避免 canvas 被拉伸导致模糊。
        resolution: window.devicePixelRatio || 1,
        autoDensity: true,
      });
      app.view.style.display = "block";
      container.appendChild(app.view);
      appRef.current = app;
    } catch (e) {
      console.error("PIXI 初始化失败:", e);
      onErrorRef.current?.(e instanceof Error ? e : new Error(String(e)));
      return;
    }

    return () => {
      modelRef.current?.destroy();
      modelRef.current = null;
      appRef.current = null;
      app?.destroy(true, { children: true });
    };
  }, []);

  // 尺寸变化：只 resize 渲染器并重新布局已有模型。
  useEffect(() => {
    const app = appRef.current;
    if (!app) return;
    app.renderer.resize(width, height);
    if (modelRef.current) {
      layoutModel(modelRef.current, width, height, modelScale);
    }
  }, [width, height, modelScale]);

  // 加载 / 切换模型（不依赖尺寸，尺寸变化不会重载模型）。
  useEffect(() => {
    // modelUrl 变为 null（如移除 active 伙伴清屏）：销毁旧模型，避免桌宠残留上一模型。
    if (!modelUrl) {
      modelRef.current?.destroy();
      modelRef.current = null;
      return;
    }
    const app = appRef.current;
    if (!app) return;
    let cancelled = false;

    void (async () => {
      modelRef.current?.destroy();
      modelRef.current = null;
      try {
        // 显式关闭 autoInteract：原版默认值是 true（眼睛跟随鼠标 + 点击触发动作），
        // 必须显式传 false 才能关闭；呼吸/眨眼等自动动画仍由 PIXI ticker 驱动。
        const model = await Live2DModel.from(modelUrl, { autoInteract: false });
        if (cancelled) {
          model.destroy();
          return;
        }
        app.stage.addChild(model);
        modelRef.current = model;
        layoutModel(model, sizeRef.current.width, sizeRef.current.height, modelScaleRef.current);
        const bounds = computeModelBounds(model);
        const valid =
          Number.isFinite(bounds.width) &&
          Number.isFinite(bounds.height) &&
          bounds.width > 0 &&
          bounds.height > 0;
        if (valid) {
          onModelMetricsRef.current?.({ aspectRatio: bounds.width / bounds.height });
        }
        // 模型已加载、画布可用：通知上层（注意画布可能尚未渲染本帧，上层截图前需等一帧）。
        onModelReadyRef.current?.(app.view as HTMLCanvasElement);
      } catch (e) {
        console.error("Live2D 模型加载失败:", e);
        onErrorRef.current?.(e instanceof Error ? e : new Error(String(e)));
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [modelUrl]);

  return <div ref={containerRef} className={className} />;
}
