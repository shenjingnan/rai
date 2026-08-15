import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import { Live2dStage } from "@/components/live2d/Live2dStage";
import { useLive2dConfig } from "@/hooks/useLive2dConfig";
import { api, onLive2dModelChanged, toAssetUrl } from "@/lib/tauri";

/** 角色窗口逻辑尺寸（与后端 `inner_size` 保持一致）。 */
const WIDTH = 360;
const HEIGHT = 480;
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
 * - 按住左键拖动移动窗口；右键弹出上下文菜单（打开设置 / 隐藏角色 / 退出）。
 */
export function CompanionRoot() {
  const containerRef = useRef<HTMLDivElement>(null);
  const { config } = useLive2dConfig();
  const [modelUrl, setModelUrl] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

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

  const items: ContextMenuItem[] = [
    { label: "打开设置", action: () => void api.openSettings() },
    { label: "隐藏角色", action: () => void api.hideCompanion() },
    { label: "退出", action: () => void api.quitApp() },
  ];

  const menuLeft = menu ? Math.min(menu.x, WIDTH - MENU_WIDTH) : 0;
  const menuTop = menu ? Math.min(menu.y, HEIGHT - MENU_HEIGHT) : 0;

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
      <Live2dStage modelUrl={modelUrl} width={WIDTH} height={HEIGHT} />

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
