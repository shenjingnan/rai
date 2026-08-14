import { Live2DModel } from "@naari3/pixi-live2d-display";
import * as PIXI from "pixi.js";
import { useEffect, useRef, useState } from "react";

// @naari3 fork 依赖全局 window.PIXI.Ticker 驱动渲染循环。
if (typeof window !== "undefined") {
  (window as unknown as { PIXI: typeof PIXI }).PIXI = PIXI;
}

interface Live2dStageProps {
  /** 模型清单文件的 asset:// URL，null 时不加载。 */
  modelUrl: string | null;
  width: number;
  height: number;
  className?: string;
  /** 模型加载成功后的回调（注意：传入时用 useCallback 包裹，避免重复触发加载）。 */
  onModelReady?: (model: Live2DModel) => void;
}

/**
 * Live2D 渲染组件：命令式创建 PIXI Application 与 canvas，
 * 规避 React StrictMode 双挂载时 PIXI 移除 DOM 节点导致引用失效的问题。
 */
export function Live2dStage({
  modelUrl,
  width,
  height,
  className,
  onModelReady,
}: Live2dStageProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<PIXI.Application | null>(null);
  const modelRef = useRef<Live2DModel | null>(null);
  const [appReady, setAppReady] = useState(false);

  // 创建 / 销毁 PIXI 应用（canvas 不写 JSX）。
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    let disposed = false;
    let app: PIXI.Application | null = null;

    void (async () => {
      app = new PIXI.Application();
      await app.init({ width, height, backgroundAlpha: 0 });
      if (disposed) {
        app.destroy(true, { children: true });
        return;
      }
      container.appendChild(app.canvas);
      appRef.current = app;
      setAppReady(true);
    })();

    return () => {
      disposed = true;
      setAppReady(false);
      appRef.current = null;
      modelRef.current = null;
      if (app) app.destroy(true, { children: true });
    };
  }, [width, height]);

  // 加载 / 切换模型。
  useEffect(() => {
    if (!appReady || !modelUrl) return;
    const app = appRef.current;
    if (!app) return;
    let cancelled = false;

    void (async () => {
      modelRef.current?.destroy();
      modelRef.current = null;
      try {
        const model = await Live2DModel.from(modelUrl, { autoInteract: true });
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
        onModelReady?.(model);
      } catch (e) {
        console.error("Live2D 模型加载失败:", e);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [appReady, modelUrl, width, height, onModelReady]);

  return <div ref={containerRef} className={className} />;
}
