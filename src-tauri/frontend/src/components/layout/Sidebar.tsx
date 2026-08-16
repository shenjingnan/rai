import { getCurrentWindow } from "@tauri-apps/api/window";
import { Home, Layers, MessageCircle, Minus, Settings, Square, Users, X } from "lucide-react";
import { useMemo, useState } from "react";
import { useLocation } from "react-router-dom";
import { Live2dStage } from "@/components/live2d/Live2dStage";
import { toAssetUrl } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";
import { NavItem } from "./NavItem";

const PRIMARY_NAV = [
  { to: "/home", icon: Home, label: "概览", end: true },
  { to: "/chat", icon: MessageCircle, label: "对话" },
  { to: "/companion", icon: Users, label: "伙伴" },
  { to: "/models", icon: Layers, label: "模型" },
  { to: "/settings", icon: Settings, label: "设置", end: true },
];

/** 左上角窗口按钮样式（非 macOS 自绘三键）。 */
const windowButtonClass =
  "flex h-full w-9 items-center justify-center text-muted-foreground transition-colors hover:bg-accent hover:text-foreground";

/** 左侧导航：左上角窗口按钮 + 真实 logo + 主导航，底部展示监听状态。 */
export function Sidebar() {
  const { live2d } = useRuntime();
  const location = useLocation();
  const [isMac] = useState(() => navigator.userAgent.includes("Macintosh"));

  const { modelUrl } = live2d.model;
  const { config } = live2d.config;

  // 优先当前会话加载的模型，其次回退到持久化配置里的模型（与 Live2dCard 保持一致）。
  const companionUrl = useMemo(() => {
    if (modelUrl) return modelUrl;
    if (config?.models_present && config.model_file) {
      return toAssetUrl(config.model_file);
    }
    return null;
  }, [modelUrl, config]);

  return (
    <aside
      data-tauri-drag-region="deep"
      className="flex w-[248px] shrink-0 flex-col bg-sidebar-background"
    >
      {/* 左上角窗口按钮区：macOS 由系统原生绘制红绿灯，此处仅留白；其它平台自绘三键。 */}
      <div
        className="flex h-8 shrink-0 items-center pl-3"
        style={isMac ? { paddingLeft: "78px" } : undefined}
      >
        {!isMac && (
          <>
            <button
              type="button"
              aria-label="最小化"
              className={windowButtonClass}
              onClick={() => getCurrentWindow().minimize()}
            >
              <Minus className="h-4 w-4" />
            </button>
            <button
              type="button"
              aria-label="最大化"
              className={windowButtonClass}
              onClick={() => getCurrentWindow().toggleMaximize()}
            >
              <Square className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              aria-label="关闭"
              className="flex h-full w-9 items-center justify-center text-muted-foreground transition-colors hover:bg-red-600 hover:text-white"
              onClick={() => getCurrentWindow().close()}
            >
              <X className="h-4 w-4" />
            </button>
          </>
        )}
      </div>

      <div className="flex items-center justify-center px-6 pt-1">
        <img src="/logo.svg" alt="ZapMomo" className="h-20 w-20" />
      </div>

      <nav className="mt-5 flex flex-col gap-1.5 px-4">
        {PRIMARY_NAV.map((item) => (
          <NavItem key={item.to} to={item.to} icon={item.icon} label={item.label} end={item.end} />
        ))}
      </nav>

      <div className="mt-auto flex flex-col gap-3 px-4 pb-6">
        {/* pixi-live2d-display 全局共享 WebGL context，同一窗口内两个 Live2D 画布会互相覆盖。
            伙伴页已有 Live2dCard 大预览，故该页隐藏侧边栏小预览，避免两个模型并存。 */}
        {companionUrl && !location.pathname.startsWith("/companion") && (
          <Live2dStage modelUrl={companionUrl} width={128} height={160} className="self-center" />
        )}
      </div>
    </aside>
  );
}
