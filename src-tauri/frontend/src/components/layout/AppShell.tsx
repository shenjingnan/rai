import { Header } from "@/components/Header";
import { cn } from "@/lib/utils";
import { MainPanel } from "./MainPanel";
import { Sidebar } from "./Sidebar";

/** 统一 App Shell：融入式标题栏 + 左侧 Sidebar + 右侧白色圆角 MainPanel。 */
export function AppShell() {
  // macOS 为非透明原生窗口，圆角由系统绘制；其它平台仍是透明窗口，需 CSS 圆角裁出。
  const isMac = navigator.userAgent.includes("Macintosh");

  return (
    <div
      className={cn(
        "flex h-screen flex-col overflow-hidden bg-app-background text-foreground",
        !isMac && "rounded-xl",
      )}
    >
      <Header />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <MainPanel />
      </div>
    </div>
  );
}
