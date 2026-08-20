import { isMacOs, isWindows } from "@/lib/platform";
import { cn } from "@/lib/utils";
import { MainPanel } from "./MainPanel";
import { Sidebar } from "./Sidebar";
import { WindowControls } from "./WindowControls";

/** 统一 App Shell：无标题栏。
 * macOS 红绿灯由系统绘制（Sidebar 左上留白）；Linux/Windows 顶部透明拖拽条
 * 拉通全宽（整条可拖动窗口），三键靠右。 */
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
      {/* Linux/Windows：透明悬浮顶部条拉通全宽（整条可拖拽窗口），三键靠右，
          不占布局、无标题栏背景。 */}
      {!mac && (
        <div
          data-tauri-drag-region
          className="absolute left-0 right-0 top-0 z-10 flex h-9 items-center justify-end"
        >
          <WindowControls />
        </div>
      )}
      <Sidebar />
      <MainPanel />
    </div>
  );
}
