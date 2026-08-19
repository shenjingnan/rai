import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "@/components/ui/toast";
import type { CompanionLibraryView, CompanionModelInfo } from "@/types/tauri";
import { CompanionPage } from "./CompanionPage";

const { invokeMock, openMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: openMock,
}));

// Live2dStage 依赖 pixi / WebGL，jsdom 无法运行；预览容器量测（ResizeObserver）在 jsdom
// 是空桩、尺寸保持 0，本组件不会真正渲染 Live2dStage，这里仍 mock 掉避免模块加载副作用。
vi.mock("@/components/live2d/Live2dStage", () => ({
  Live2dStage: () => <div data-testid="live2d-stage" />,
}));

function model(id: string, name: string, valid = true): CompanionModelInfo {
  return {
    id,
    name,
    source_path: `/src/${name}`,
    model_dir: `/zap/.zapmomo/companions/${id}`,
    model_file: `/zap/.zapmomo/companions/${id}/${name}.model3.json`,
    format: "cubism3",
    imported_at: "2026-01-01T00:00:00Z",
    valid,
    cover_image: null,
  };
}

const MODEL_A = model("companion-aaaa", "大月下");
const MODEL_B = model("companion-bbbb", "星语");

/** 可变伙伴库快照（模拟后端 list/import/set_active 语义）。 */
let library: CompanionLibraryView;
/** import_companion mock 用序号生成唯一 id。 */
let importSeq: number;

beforeEach(() => {
  invokeMock.mockReset();
  openMock.mockReset();
  library = { models: [], active_model_id: null };
  importSeq = 0;

  invokeMock.mockImplementation(
    (cmd: string, args?: { sourceDir?: string; id?: string; name?: string }) => {
      switch (cmd) {
        case "list_companions":
          return Promise.resolve(library);
        case "get_live2d_config":
          return Promise.resolve({
            model_dir: null,
            model_file: null,
            format: null,
            models_present: false,
            window_scale: 1.0,
            window_opacity: 1.0,
            settings_path: "/zap/.zapmomo/settings.toml",
          });
        case "import_companion": {
          const sourceDir = args?.sourceDir ?? "";
          const name = sourceDir.split("/").pop() ?? "模型";
          const existing = library.models.find((m) => m.source_path === sourceDir);
          if (existing) {
            return Promise.resolve({ library, model_id: existing.id, already_imported: true });
          }
          const id = `companion-import-${++importSeq}`;
          const first = library.models.length === 0;
          const imported = { ...model(id, name), source_path: sourceDir };
          library = {
            models: [...library.models, imported],
            // 首次导入自动 active（与后端 import_from_dir 语义一致）。
            active_model_id: first ? id : library.active_model_id,
          };
          return Promise.resolve({ library, model_id: id, already_imported: false });
        }
        case "set_active_companion": {
          library = { ...library, active_model_id: args?.id ?? null };
          return Promise.resolve(library);
        }
        case "rename_companion": {
          library = {
            ...library,
            models: library.models.map((m) =>
              m.id === args?.id ? { ...m, name: args?.name ?? m.name } : m,
            ),
          };
          return Promise.resolve(library);
        }
        case "remove_companion": {
          const id = args?.id ?? "";
          const remaining = library.models.filter((m) => m.id !== id);
          library = {
            models: remaining,
            // 删的是 active → 落到第一个剩余或 null（与后端语义一致）。
            active_model_id:
              library.active_model_id === id ? (remaining[0]?.id ?? null) : library.active_model_id,
          };
          return Promise.resolve(library);
        }
        default:
          return Promise.resolve(undefined);
      }
    },
  );
});

function renderPage() {
  return render(
    <ToastProvider>
      <CompanionPage />
    </ToastProvider>,
  );
}

describe("CompanionPage 伙伴模型管理器", () => {
  it("选中伙伴后显示尺寸控制，拖动滑块调用 set_companion_scale", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    await screen.findByRole("button", { name: /大月下.*使用中/ });
    // 初始从 get_live2d_config 读到 window_scale=1.0 → 100%（异步等待出现）。
    expect(await screen.findByText("尺寸")).toBeInTheDocument();
    // 尺寸与透明度两个滑块初始都是 100%。
    expect(await screen.findAllByText("100%")).toHaveLength(2);

    // 键盘微调滑块（Radix Slider role="slider"）：每次步进 5。
    const slider = screen.getByRole("slider", { name: "尺寸" });
    slider.focus();
    await user.keyboard("{ArrowRight}");
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_companion_scale", { scale: expect.any(Number) });
    });
  });

  it("选中伙伴后显示透明度控制，拖动滑块调用 set_companion_opacity", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    await screen.findByRole("button", { name: /大月下.*使用中/ });
    expect(await screen.findByText("透明度")).toBeInTheDocument();
    expect(await screen.findAllByText("100%")).toHaveLength(2);

    const slider = screen.getByRole("slider", { name: "透明度" });
    slider.focus();
    await user.keyboard("{ArrowLeft}"); // 100 → 95
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_companion_opacity", {
        opacity: expect.any(Number),
      });
    });
  });

  it("空库时显示空态", async () => {
    renderPage();
    expect(await screen.findByText("还没有伙伴")).toBeInTheDocument();
    expect(screen.getAllByText("暂无伙伴").length).toBeGreaterThanOrEqual(1);
    // 顶部有主导入入口（左侧底部按钮已移除）
    expect(screen.getByRole("button", { name: "添加伙伴" })).toBeInTheDocument();
  });

  it("列出伙伴；默认选中 active；点击其他伙伴仅切换预览不切换 active", async () => {
    library = { models: [MODEL_A, MODEL_B], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    // 默认 selected = active（大月下）：列表项有「使用中」徽标。
    expect(await screen.findByText("使用中")).toBeInTheDocument();

    // 点击星语 → 预览切换为「设为当前使用」，active 仍是 大月下。
    await user.click(screen.getByRole("button", { name: MODEL_B.name }));
    expect(screen.getByRole("button", { name: "设为当前使用" })).toBeEnabled();
    // 蓝色选中边框落到星语（行容器上）。
    expect(screen.getByTestId(`companion-item-${MODEL_B.id}`)).toHaveClass("border-primary/60");
    // 「使用中」徽标仍在大月下列表项上（只有 1 个）。
    expect(screen.getAllByText("使用中")).toHaveLength(1);
    expect(screen.getByTestId(`companion-item-${MODEL_A.id}`)).toHaveTextContent("使用中");
  });

  it("设为当前使用：点击后 active 持久化到后端并更新 Badge", async () => {
    library = { models: [MODEL_A, MODEL_B], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: MODEL_B.name }));
    await user.click(screen.getByRole("button", { name: "设为当前使用" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_active_companion", { id: MODEL_B.id });
    });
    // active 变为星语：右侧 CTA 消失（已是当前使用），「使用中」徽标落到星语。
    await waitFor(() => {
      expect(screen.getByTestId(`companion-item-${MODEL_B.id}`)).toHaveTextContent("使用中");
    });
    expect(screen.queryByRole("button", { name: "设为当前使用" })).not.toBeInTheDocument();
    expect(screen.getAllByText("使用中")).toHaveLength(1);
  });

  it("首次导入：自动 selected + active，右侧直接显示「当前使用」", async () => {
    openMock.mockResolvedValue("/Downloads/星语");
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "添加伙伴" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("import_companion", {
        sourceDir: "/Downloads/星语",
      });
    });
    expect(await screen.findByText("✓ 已导入「星语」")).toBeInTheDocument();
    // 首次导入自动 active：新伙伴带「使用中」徽标，右侧无 CTA。
    expect(await screen.findByText("使用中")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "设为当前使用" })).not.toBeInTheDocument();
  });

  it("第二次导入（已有 active）：selected 变新模型，active 不变", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    openMock.mockResolvedValue("/Downloads/星语");
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "添加伙伴" }));

    // selected = 新模型（星语），active 仍是大月下 → 预览显示「设为当前使用」。
    expect(await screen.findByRole("button", { name: "设为当前使用" })).toBeEnabled();
    // 「使用中」徽标仍在大月下（列表项）上。
    expect(screen.getAllByText("使用中")).toHaveLength(1);
    expect(screen.getByTestId(`companion-item-${MODEL_A.id}`)).toHaveTextContent("使用中");
  });

  it("重复导入同一目录：提示已导入，不新增伙伴", async () => {
    openMock.mockResolvedValue("/Downloads/星语");
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "添加伙伴" }));
    await screen.findByText("✓ 已导入「星语」");

    await user.click(screen.getByRole("button", { name: "添加伙伴" }));
    expect(await screen.findByText("该伙伴已经导入")).toBeInTheDocument();
    expect(library.models).toHaveLength(1);
  });

  it("模型不可用：列表显示「模型不可用」，预览提示无法加载，禁止设为当前", async () => {
    const broken = model("companion-broken", "莉娅", false);
    library = { models: [broken], active_model_id: null };
    const user = userEvent.setup();
    renderPage();

    expect(await screen.findByText("模型不可用")).toBeInTheDocument();
    // 列表项选择按钮的可访问名 = 「莉娅 模型不可用」（重命名铅笔的 aria-label 也含「莉娅」，
    // 需用更具体的正则区分）。
    await user.click(screen.getByRole("button", { name: /莉娅.*模型不可用/ }));
    expect(screen.getByText("无法加载该 Live2D 模型")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "设为当前使用" })).toBeDisabled();
  });

  it("取消目录选择时不导入", async () => {
    openMock.mockResolvedValue(null);
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "添加伙伴" }));
    await waitFor(() => {
      expect(openMock).toHaveBeenCalled();
    });
    expect(invokeMock).not.toHaveBeenCalledWith("import_companion", expect.anything());
  });

  it("重命名伙伴：铅笔编辑后调用 rename_companion 并更新列表", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    // active 模型的选择按钮可访问名 = 「大月下 使用中」。
    await screen.findByRole("button", { name: /大月下.*使用中/ });
    // 铅笔（hover 图标始终在 DOM）→ 行内输入框。
    await user.click(screen.getByRole("button", { name: `重命名「${MODEL_A.name}」` }));
    const input = await screen.findByDisplayValue(MODEL_A.name);
    await user.clear(input);
    await user.type(input, "大月下改名");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("rename_companion", {
        id: MODEL_A.id,
        name: "大月下改名",
      });
    });
    expect(await screen.findByText("✓ 已重命名为「大月下改名」")).toBeInTheDocument();
    // 列表更新为新名字（徽标不变，仍使用中）。
    expect(screen.getByTestId(`companion-item-${MODEL_A.id}`)).toHaveTextContent("大月下改名");
    expect(screen.getByTestId(`companion-item-${MODEL_A.id}`)).toHaveTextContent("使用中");
  });

  it("有封面图时列表显示封面，无封面图用占位图标", async () => {
    const withCover = {
      ...MODEL_A,
      cover_image: "/zap/.zapmomo/companions/companion-aaaa/preview.png",
    };
    library = { models: [withCover, MODEL_B], active_model_id: null };
    // 封面图 alt="" 属装饰性图片，无障碍树不带 img role，用 container 直接查 img 元素。
    const { container } = renderPage();

    await screen.findByRole("button", { name: MODEL_B.name });
    const imgs = container.querySelectorAll("img");
    expect(imgs).toHaveLength(1);
    expect(imgs[0]).toHaveAttribute("src", expect.stringContaining("asset://localhost"));
  });

  it("移除伙伴：垃圾桶 → 确认对话框 → remove_companion，列表移除且 active 不变", async () => {
    library = { models: [MODEL_A, MODEL_B], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    await screen.findByRole("button", { name: /大月下.*使用中/ });
    // 移除非使用中的星语。
    await user.click(screen.getByRole("button", { name: `移除「${MODEL_B.name}」` }));
    const dialog = screen.getByRole("dialog", { name: "移除伙伴" });
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).getByText(/确定要移除/)).toBeInTheDocument();
    expect(within(dialog).getByText("星语")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "移除" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("remove_companion", { id: MODEL_B.id });
    });
    // 确认后弹窗应真正关闭（LibraryDialog open→false 会退出动画后卸载）。
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "移除伙伴" })).not.toBeInTheDocument();
    });
    // 星语从列表消失，大月下仍是使用中。
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: MODEL_B.name })).not.toBeInTheDocument();
    });
    expect(await screen.findByText("✓ 已移除「星语」")).toBeInTheDocument();
    expect(screen.getByTestId(`companion-item-${MODEL_A.id}`)).toHaveTextContent("使用中");
    expect(screen.getAllByText("使用中")).toHaveLength(1);
  });

  it("使用中的伙伴删除按钮被禁用，点击不弹确认框", async () => {
    library = { models: [MODEL_A, MODEL_B], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    await screen.findByRole("button", { name: /大月下.*使用中/ });
    // 大月下（使用中）删除按钮禁用；星语可用。
    expect(screen.getByRole("button", { name: `移除「${MODEL_A.name}」` })).toBeDisabled();
    expect(screen.getByRole("button", { name: `移除「${MODEL_B.name}」` })).toBeEnabled();

    await user.click(screen.getByRole("button", { name: `移除「${MODEL_A.name}」` }));
    expect(screen.queryByRole("dialog", { name: "移除伙伴" })).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("remove_companion", expect.anything());
  });

  it("移除时点取消，不调用 remove_companion", async () => {
    library = { models: [MODEL_A, MODEL_B], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    await screen.findByRole("button", { name: /大月下.*使用中/ });
    await user.click(screen.getByRole("button", { name: `移除「${MODEL_B.name}」` }));
    await user.click(screen.getByRole("button", { name: "取消" }));

    expect(invokeMock).not.toHaveBeenCalledWith("remove_companion", expect.anything());
    // 取消后弹窗应真正关闭（不再残留空内容弹窗）。
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "移除伙伴" })).not.toBeInTheDocument();
    });
  });

  it("重命名按 Escape 取消，不调用后端", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    await screen.findByRole("button", { name: /大月下.*使用中/ });
    await user.click(screen.getByRole("button", { name: `重命名「${MODEL_A.name}」` }));
    await user.keyboard("{Escape}");

    expect(invokeMock).not.toHaveBeenCalledWith("rename_companion", expect.anything());
  });
});
