import { isMacOs, isWindows } from "@/lib/platform";
import { cn } from "@/lib/utils";
import { MainPanel } from "./MainPanel";
import { Sidebar } from "./Sidebar";
import { WindowControls } from "./WindowControls";

/** 统一 App Shell：无标题栏。
 * 三平台顶部透明拖拽条拉通全宽（整条可拖动窗口）：Linux/Windows 三键靠右；
 * macOS 无三键（系统红绿灯为原生控件，层级在 webview 之上不受影响；
 * Overlay 标题栏下原生拖拽区被 webview 覆盖，拖拽须由 HTML 拖拽区承担）。 */
export function AppShell() {
  const mac = isMacOs();
  const windows = isWindows();

  return (
    <div
      className={cn(
        "relative flex h-screen overflow-hidden bg-app-background text-foreground",
        // 圆角：macOS 由系统绘制；Linux 透明窗口需 CSS 圆角裁出；Windows 不透明方角无需处理。
        !mac && !windows && "rounded-xl",
        // Windows：后端已关 DWM shadow（undecorated+shadow 在 Win10 会被 DWM 画成
        // 左右底三边黑框、顶部强制无边），四边完整边框由 CSS 自绘。
        windows && "border border-window-border",
      )}
    >
      {/* 三平台：透明悬浮顶部条拉通全宽（整条可拖拽窗口），三键靠右（仅非 macOS），
          不占布局、无标题栏背景。 */}
      <div
        data-tauri-drag-region
        className="absolute left-0 right-0 top-0 z-10 flex h-9 items-center justify-end"
      >
        {!mac && <WindowControls />}
      </div>
      <Sidebar />
      <MainPanel />
    </div>
  );
}
