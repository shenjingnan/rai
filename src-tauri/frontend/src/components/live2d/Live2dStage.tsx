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
  /** 渲染初始化或模型加载失败时的回调。 */
  onError?: (error: Error) => void;
}

/**
 * Live2D 渲染组件：命令式创建 PIXI Application（PIXI 6 同步构造），
 * 规避 React StrictMode 双挂载时 PIXI 移除 DOM 节点导致引用失效的问题。
 */
export function Live2dStage({ modelUrl, width, height, className, onError }: Live2dStageProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<PIXI.Application | null>(null);
  const modelRef = useRef<Live2DModel | null>(null);

  // 创建 / 销毁 PIXI 应用。
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let app: PIXI.Application | null = null;
    try {
      app = new PIXI.Application({
        width,
        height,
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
      onError?.(e instanceof Error ? e : new Error(String(e)));
      return;
    }

    return () => {
      modelRef.current?.destroy();
      modelRef.current = null;
      appRef.current = null;
      app?.destroy(true, { children: true });
    };
  }, [width, height, onError]);

  // 加载 / 切换模型。
  useEffect(() => {
    if (!modelUrl) return;
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
        // 缩放居中到画布内，留 20% 边距。
        const scale = Math.min(width / model.width, height / model.height) * 0.8;
        model.scale.set(scale);
        model.anchor.set(0.5, 0.5);
        model.position.set(width / 2, height / 2);
        app.stage.addChild(model);
        modelRef.current = model;
      } catch (e) {
        console.error("Live2D 模型加载失败:", e);
        onError?.(e instanceof Error ? e : new Error(String(e)));
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [modelUrl, width, height, onError]);

  return <div ref={containerRef} className={className} />;
}
