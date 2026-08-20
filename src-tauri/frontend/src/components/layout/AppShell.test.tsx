import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { AppShell } from "./AppShell";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  })),
}));

// 与真实平台一致的 userAgent 样本（平台判定基于 userAgent 子串匹配）。
const WINDOWS_UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";
const LINUX_UA =
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const MAC_UA =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";

/** 临时覆盖 navigator.userAgent 渲染 AppShell（结束后恢复，避免泄漏到其它用例）。 */
function renderShellWithUserAgent(ua: string) {
  const desc = Object.getOwnPropertyDescriptor(navigator, "userAgent");
  Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });
  try {
    return render(
      <MemoryRouter>
        <Routes>
          <Route path="/" element={<AppShell />}>
            <Route index element={<div>page</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );
  } finally {
    if (desc) Object.defineProperty(navigator, "userAgent", desc);
  }
}

/** 自绘窗口三键按钮（最小化/最大化/关闭）。 */
function queryWindowButtons() {
  return {
    minimize: screen.queryByRole("button", { name: "最小化" }),
    maximize: screen.queryByRole("button", { name: "最大化" }),
    close: screen.queryByRole("button", { name: "关闭" }),
  };
}

/** AppShell 根元素（render 容器的第一个子节点）。 */
function shellRoot(container: HTMLElement) {
  return container.firstElementChild as HTMLElement;
}

/** 悬浮三键条：AppShell 根下 absolute 定位到右上角的拖拽区容器。 */
function floatingControlsBar(root: HTMLElement) {
  return Array.from(root.children).find(
    (el) =>
      el instanceof HTMLElement &&
      el.hasAttribute("data-tauri-drag-region") &&
      el.className.includes("absolute") &&
      el.className.includes("top-0") &&
      el.className.includes("right-0"),
  );
}

/** 主面板外层容器（AppShell 根的最后一个子节点，Sidebar 之后的右侧区域）。 */
function mainPanelOuter(root: HTMLElement) {
  return root.lastElementChild as HTMLElement;
}

describe("AppShell 窗口控件的平台布局", () => {
  it("Windows：三键悬浮在右上角（无标题栏），窗口无 CSS 圆角，主面板顶部让位", () => {
    const { container } = renderShellWithUserAgent(WINDOWS_UA);
    const root = shellRoot(container);

    // 三键渲染，且不在侧边栏内（悬浮于主面板上方，而非左上角）
    const buttons = queryWindowButtons();
    expect(buttons.minimize).not.toBeNull();
    expect(buttons.maximize).not.toBeNull();
    expect(buttons.close).not.toBeNull();
    expect(buttons.minimize?.closest("aside")).toBeNull();

    // 悬浮条存在且容纳三键（可拖拽窗口）
    const bar = floatingControlsBar(root);
    expect(bar).toBeDefined();
    expect(bar?.contains(buttons.close as HTMLElement)).toBe(true);

    // Windows 不做 CSS 圆角裁剪
    expect(root.className).not.toContain("rounded-xl");

    // 主面板顶部让出三键高度，避免内容被遮挡
    expect(mainPanelOuter(root).className).toContain("pt-9");
  });

  it("Linux：三键与 Windows 一致悬浮右上角，保留 CSS 圆角，主面板顶部让位", () => {
    const { container } = renderShellWithUserAgent(LINUX_UA);
    const root = shellRoot(container);

    // 三键渲染，且不在侧边栏内（悬浮于主面板上方，而非左上角）
    const buttons = queryWindowButtons();
    expect(buttons.minimize).not.toBeNull();
    expect(buttons.maximize).not.toBeNull();
    expect(buttons.close).not.toBeNull();
    expect(buttons.minimize?.closest("aside")).toBeNull();

    // 悬浮条存在且容纳三键（可拖拽窗口）
    const bar = floatingControlsBar(root);
    expect(bar).toBeDefined();
    expect(bar?.contains(buttons.close as HTMLElement)).toBe(true);

    // 透明窗口圆角仍由 CSS 裁出
    expect(root.className).toContain("rounded-xl");

    // 主面板顶部让出三键高度，避免内容被遮挡
    expect(mainPanelOuter(root).className).toContain("pt-9");
  });

  it("macOS：不自绘三键（系统红绿灯），无 CSS 圆角", () => {
    const { container } = renderShellWithUserAgent(MAC_UA);
    const root = shellRoot(container);

    const buttons = queryWindowButtons();
    expect(buttons.minimize).toBeNull();
    expect(buttons.maximize).toBeNull();
    expect(buttons.close).toBeNull();

    expect(root.className).not.toContain("rounded-xl");
  });
});
