import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { useState } from "react";

const windowButtonClass =
  "flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-accent hover:text-foreground";

/**
 * 融入式标题栏：仅提供拖拽与窗口控制，不显示标题文字（logo 已在 Sidebar）。
 * macOS 保留系统红绿灯留白；非 macOS 自绘最小化/最大化/关闭按钮。
 */
export function Header() {
  const [isMac] = useState(() => navigator.userAgent.includes("Macintosh"));

  return (
    <header
      data-tauri-drag-region
      className="flex h-12 shrink-0 select-none items-center bg-app-background pr-1"
      style={isMac ? { paddingLeft: "78px" } : undefined}
    >
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
