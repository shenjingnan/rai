import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "@/components/ui/toast";
import type { StorageInfo, StorageMigrateProgress } from "@/types/modelLibrary";
import { SettingsPage } from "./SettingsPage";

const { invokeMock, openMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

/** 事件处理器缓存：测试可向组件推事件（如迁移进度）。 */
const listenHandlers = new Map<string, (payload: unknown) => void>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, cb: (e: { payload: unknown }) => void) => {
    listenHandlers.set(event, (payload) => cb({ payload }));
    return Promise.resolve(() => {});
  },
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: openMock,
}));

vi.mock("@/providers/RuntimeContext", () => ({
  useRuntime: () => ({
    devices: { devices: [], error: null, refresh: vi.fn() },
    device: null,
    setDevice: vi.fn(),
    anyListening: false,
  }),
}));

function storageInfo(overrides: Partial<StorageInfo> = {}): StorageInfo {
  return {
    dataDir: null,
    modelsDir: "/zap/.zapmomo/models",
    companionsDir: "/zap/.zapmomo/companions",
    legacyModelsDir: "/zap/.zapmomo/models",
    legacyCompanionsDir: null,
    legacyModelsBytes: 1024,
    legacyCompanionsBytes: 0,
    migrationAvailable: true,
    migrating: false,
    sameVolume: true,
    diskTotal: 1000,
    diskAvailable: 500,
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
  openMock.mockReset();
  listenHandlers.clear();
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_hide_dock_icon":
        return Promise.resolve(false);
      case "get_shortcuts":
        return Promise.resolve({});
      case "catalog_get_endpoint":
        return Promise.resolve({ downloadSource: "auto", mirrorUrl: "" });
      case "get_storage_info":
        return Promise.resolve(storageInfo());
      default:
        return Promise.resolve();
    }
  });
});

function renderPage() {
  return render(
    <ToastProvider>
      <SettingsPage />
    </ToastProvider>,
  );
}

describe("SettingsPage 存储位置", () => {
  it("渲染数据目录与迁移行", async () => {
    renderPage();
    expect(await screen.findByText("存储位置")).toBeInTheDocument();
    expect(await screen.findByText("/zap/.zapmomo/models")).toBeInTheDocument();
    expect(await screen.findByText("开始迁移")).toBeInTheDocument();
    expect(screen.getByText(/旧目录占用/)).toBeInTheDocument();
  });

  it("选目录后调用 set_data_dir 并刷新", async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText("存储位置");

    openMock.mockResolvedValue("/new/data/dir");
    await user.click(await screen.findByRole("button", { name: "更改" }));
    // 确认框 → 选择目录
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "选择目录" }));

    // 确认框关闭后触发 open()，再 set_data_dir
    await waitFor(() => expect(openMock).toHaveBeenCalled());
    expect(invokeMock).toHaveBeenCalledWith("set_data_dir", { path: "/new/data/dir" });
  });

  it("迁移进度条随事件更新并可取消", async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText("存储位置");

    await user.click(await screen.findByRole("button", { name: "开始迁移" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "开始迁移" }));

    // 推一条 moving 进度
    const progress: StorageMigrateProgress = {
      state: "moving",
      currentItem: "model-a",
      itemsDone: 1,
      itemsTotal: 3,
      bytesDone: 512,
      bytesTotal: 2048,
      message: "正在迁移 model-a",
      failedItems: [],
    };
    listenHandlers.get("storage-migrate-progress")?.(progress);

    expect(await screen.findByText("正在迁移 model-a")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "取消迁移" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "取消迁移" }));
    expect(invokeMock).toHaveBeenCalledWith("cancel_storage_migration");
  });

  it("迁移完成事件后刷新存储信息", async () => {
    renderPage();
    await screen.findByText("存储位置");

    const progress: StorageMigrateProgress = {
      state: "done",
      currentItem: null,
      itemsDone: 3,
      itemsTotal: 3,
      bytesDone: 2048,
      bytesTotal: 2048,
      message: "迁移完成",
      failedItems: [],
    };
    listenHandlers.get("storage-migrate-progress")?.(progress);
    // 完成事件应触发刷新（再次 get_storage_info）
    await waitFor(() => {
      const calls = invokeMock.mock.calls.filter(([c]) => c === "get_storage_info");
      expect(calls.length).toBeGreaterThanOrEqual(2);
    });
  });

  it("无存量时迁移行不渲染", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "get_storage_info":
          return Promise.resolve(storageInfo({ migrationAvailable: false }));
        case "catalog_get_endpoint":
          return Promise.resolve({ downloadSource: "auto", mirrorUrl: "" });
        default:
          return Promise.resolve();
      }
    });
    renderPage();
    await screen.findByText("存储位置");
    expect(screen.queryByRole("button", { name: "开始迁移" })).not.toBeInTheDocument();
  });
});
