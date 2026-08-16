import { Outlet } from "react-router-dom";

/** 右侧主内容面板：白色大圆角 + 轻边框 + 轻阴影，内嵌路由出口。 */
export function MainPanel() {
  return (
    <div className="min-w-0 flex-1 p-3 pl-2">
      <div className="h-full overflow-hidden rounded-[18px] border border-panel-border bg-panel-background shadow-[0_1px_2px_rgba(15,23,42,0.02),0_4px_20px_rgba(15,23,42,0.03)]">
        <div className="h-full overflow-y-auto p-8">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
