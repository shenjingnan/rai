import * as PIXI from "pixi.js";
import { Live2DModel } from "pixi-live2d-display/cubism4";
import { useEffect, useRef } from "react";
import { computeModelBounds, layoutModel } from "./modelLayout";

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
