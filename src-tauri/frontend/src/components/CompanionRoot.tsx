import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import { Live2dStage } from "@/components/live2d/Live2dStage";
import { useLive2dConfig } from "@/hooks/useLive2dConfig";
import { api, onLive2dModelChanged, toAssetUrl } from "@/lib/tauri";

/** 角色窗口的基准高度（逻辑像素，与后端 `inner_size` 高度一致）；宽度按角色宽高比自适应。 */
const BASE_HEIGHT = 480;
/** 窗口宽度下限/下限比例与初始值。 */
const MIN_WIDTH = 120;
const INITIAL_WIDTH = 360;
/** 右键菜单估算尺寸（贴边时反向偏移，避免被窗口裁剪）。 */
const MENU_WIDTH = 160;
const MENU_HEIGHT = 132;

interface ContextMenuItem {
  label: string;
  action: () => void;
}

/**
 * 常驻角色窗口：静态展示 Live2D 模型（仅呼吸/眨眼等自动动画，不跟随鼠标）。
 *
 * - 启动时读 `get_live2d_config` 恢复持久化的模型（顺带重放行 asset 协议 scope）；
 * - 订阅 `live2d-model-changed`，设置窗口切换模型时即时重载；
 * - 模型加载后按角色真实宽高比自适应窗口宽度（高度固定 BASE_HEIGHT）；
 * - 按住左键拖动移动窗口；右键弹出上下文菜单（打开设置 / 隐藏角色 / 退出）。
 */
export function CompanionRoot() {
  const containerRef = useRef<HTMLDivElement>(null);
  const { config } = useLive2dConfig();
  const [modelUrl, setModelUrl] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [size, setSize] = useState({ width: INITIAL_WIDTH, height: BASE_HEIGHT });

  useEffect(() => {
    if (config?.models_present && config.model_file) {
      setModelUrl(toAssetUrl(config.model_file));
    }
  }, [config]);

  useEffect(() => {
    const unlisten = onLive2dModelChanged((info) => {
      setModelUrl(toAssetUrl(info.model_file));
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // 模型加载完成后，按角色真实宽高比自适应窗口宽度（高度固定，宽度 clamp 到屏幕可用区域）。
  const handleModelMetrics = useCallback(async (metrics: { aspectRatio: number }) => {
    const height = BASE_HEIGHT;
    let width = Math.round(height * metrics.aspectRatio);
    if (!Number.isFinite(width) || width <= 0) {
      width = INITIAL_WIDTH;
    }
    const maxWidth = Math.floor(window.screen.availWidth * 0.8);
    width = Math.max(MIN_WIDTH, Math.min(width, maxWidth));
    await getCurrentWindow().setSize(new LogicalSize(width, height));
    setSize({ width, height });
  }, []);

  const items: ContextMenuItem[] = [
    { label: "打开设置", action: () => void api.openSettings() },
    { label: "隐藏角色", action: () => void api.hideCompanion() },
    { label: "退出", action: () => void api.quitApp() },
  ];

  const menuLeft = menu ? Math.min(menu.x, size.width - MENU_WIDTH) : 0;
  const menuTop = menu ? Math.min(menu.y, size.height - MENU_HEIGHT) : 0;

  return (
    <div
      ref={containerRef}
      className="h-screen w-screen select-none overflow-hidden bg-transparent"
      onMouseDown={(e) => {
        if (e.button !== 0) return;
        void getCurrentWindow().startDragging();
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY });
      }}
    >
      <Live2dStage
        modelUrl={modelUrl}
        width={size.width}
        height={size.height}
        onModelMetrics={handleModelMetrics}
      />

      {menu && (
        <>
          {/* 半透明遮罩：点击菜单外任意处关闭（阻止冒泡到拖拽/右键处理）。 */}
          <div
            className="fixed inset-0 z-40"
            onMouseDown={(e) => {
              e.stopPropagation();
              setMenu(null);
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setMenu(null);
            }}
          />
          <div
            className="fixed z-50 min-w-36 rounded-lg border bg-card py-1 text-sm text-foreground shadow-lg"
            style={{ left: menuLeft, top: menuTop }}
            onMouseDown={(e) => e.stopPropagation()}
          >
            {items.map((item) => (
              <button
                key={item.label}
                type="button"
                className="block w-full px-3 py-1.5 text-left hover:bg-accent"
                onClick={() => {
                  item.action();
                  setMenu(null);
                }}
              >
                {item.label}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
