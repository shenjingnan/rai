import { Home, Layers, MessageCircle, Settings, Users } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { useRuntime } from "@/providers/RuntimeContext";
import { NavItem } from "./NavItem";

const PRIMARY_NAV = [
  { to: "/home", icon: Home, label: "首页", end: true },
  { to: "/chat", icon: MessageCircle, label: "对话" },
  { to: "/companion", icon: Users, label: "伙伴" },
  { to: "/models", icon: Layers, label: "模型" },
];

/** 左侧导航：真实 logo + 主导航，设置沉底，底部展示监听状态。 */
export function Sidebar() {
  const { anyListening } = useRuntime();

  return (
    <aside className="flex w-[248px] shrink-0 flex-col bg-sidebar-background">
      <div className="flex items-center px-6 pt-8">
        <img src="/logo.svg" alt="ZapMomo" className="h-9 w-9" />
      </div>

      <nav className="mt-12 flex flex-col gap-1.5 px-4">
        {PRIMARY_NAV.map((item) => (
          <NavItem key={item.to} to={item.to} icon={item.icon} label={item.label} end={item.end} />
        ))}
      </nav>

      <div className="mt-auto flex flex-col gap-3 px-4 pb-6">
        <NavItem to="/settings" icon={Settings} label="设置" end />
        <div className="flex items-center px-3">
          <Badge
            className={
              anyListening ? "bg-emerald-100 text-emerald-700" : "bg-muted text-muted-foreground"
            }
          >
            {anyListening ? "监听中" : "空闲"}
          </Badge>
        </div>
      </div>
    </aside>
  );
}
