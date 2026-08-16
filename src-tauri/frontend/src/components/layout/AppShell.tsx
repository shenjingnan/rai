import { cn } from "@/lib/utils";
import { MainPanel } from "./MainPanel";
import { Sidebar } from "./Sidebar";

/** 统一 App Shell：无全宽标题栏，窗口按钮在 Sidebar 左上角，右侧白色圆角 MainPanel。 */
export function AppShell() {
  // macOS 为非透明原生窗口，圆角由系统绘制；其它平台仍是透明窗口，需 CSS 圆角裁出。
  const isMac = navigator.userAgent.includes("Macintosh");

  return (
    <div
      className={cn(
        "flex h-screen overflow-hidden bg-app-background text-foreground",
        !isMac && "rounded-xl",
      )}
    >
      <Sidebar />
      <MainPanel />
    </div>
  );
}
