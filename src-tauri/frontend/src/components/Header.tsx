import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import type { AppInfo } from "@/types/tauri";

interface HeaderProps {
  info: AppInfo | null;
  isListening: boolean;
}

const windowButtonClass =
  "flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-accent hover:text-foreground";

/** 无边框窗口的自定义标题栏：整条可拖拽，非 macOS 平台自绘窗口控制按钮。 */
export function Header({ info, isListening }: HeaderProps) {
  // macOS 用系统红绿灯（titleBarStyle: Overlay 保留），Windows/Linux 自绘按钮
  const [isMac] = useState(() => navigator.userAgent.includes("Macintosh"));

  return (
    <header
      data-tauri-drag-region
      className="flex h-12 shrink-0 select-none items-center gap-3 border-b bg-card pl-4 pr-1"
      style={isMac ? { paddingLeft: "78px" } : undefined}
    >
      <h1 className="text-base font-semibold">ZapMomo</h1>
      {info && <span className="text-sm text-muted-foreground">v{info.version}</span>}
      <Badge
        className={
          isListening ? "bg-emerald-100 text-emerald-700" : "bg-muted text-muted-foreground"
        }
      >
        {isListening ? "监听中" : "空闲"}
      </Badge>

      {!isMac && (
        <div className="ml-auto flex h-full items-center">
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
            className="flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-red-600 hover:text-white"
            onClick={() => getCurrentWindow().close()}
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      )}
    </header>
  );
}
