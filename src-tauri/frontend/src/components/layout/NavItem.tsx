import type { ComponentType } from "react";
import { NavLink, useLocation } from "react-router-dom";
import { cn } from "@/lib/utils";

interface NavItemProps {
  to: string;
  icon: ComponentType<{ className?: string }>;
  label: string;
  /** 是否精确匹配（避免父路径误命中子路径高亮） */
  end?: boolean;
  /** 命中这些路径时强制不高亮（用于「模型」排除「模型库」子路径） */
  exclude?: string[];
}

/** Sidebar 导航项：普通透明、hover 浅蓝灰、active 浅蓝底 + 蓝色文字/图标。 */
export function NavItem({ to, icon: Icon, label, end, exclude }: NavItemProps) {
  const { pathname } = useLocation();
  const excluded = exclude?.some((p) => pathname === p || pathname.startsWith(`${p}/`)) ?? false;
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        cn(
          "flex h-11 items-center gap-3 rounded-xl px-3 text-sm font-medium transition-colors",
          isActive && !excluded
            ? "bg-nav-active text-primary"
            : "text-text-secondary hover:bg-nav-hover hover:text-text-primary",
        )
      }
    >
      <Icon className="h-[18px] w-[18px] shrink-0" />
      <span>{label}</span>
    </NavLink>
  );
}
