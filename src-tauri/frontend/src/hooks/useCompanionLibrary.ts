import { useCallback, useEffect, useState } from "react";
import { useToast } from "@/components/ui/toast";
import { api } from "@/lib/tauri";
import type { CompanionLibraryView, CompanionModelInfo } from "@/types/tauri";

export interface CompanionLibraryState {
  /** 伙伴库快照（null = 尚未加载完成）。 */
  library: CompanionLibraryView | null;
  loading: boolean;
  error: string | null;
  refreshing: boolean;
  refresh: () => Promise<void>;
  /** 导入模型目录；返回导入（或已存在）的伙伴，供页面选中。 */
  importModel: (sourceDir: string) => Promise<CompanionModelInfo | null>;
  /** 设为当前使用。 */
  setActive: (id: string) => Promise<void>;
  /** 重命名伙伴（只改展示名）。 */
  rename: (id: string, name: string) => Promise<void>;
  /** 移除伙伴（删除托管文件；若删的是 active 会自动落位/清空）。 */
  remove: (id: string) => Promise<void>;
  /** 保存从 Live2D 渲染画布截取的封面 PNG，保存后返回更新后的库视图。 */
  saveCover: (id: string, png: number[]) => Promise<CompanionLibraryView | null>;
}

/**
 * 伙伴库状态（页面级 hook，对齐 `useModelLibrary`，不进全局 RuntimeContext）。
 *
 * active 的「全局持久化」由后端 `library.json` 承担，桌宠窗口通过
 * `get_live2d_config` / `live2d-model-changed` 读取，无需前端 Context 中转。
 */
export function useCompanionLibrary(): CompanionLibraryState {
  const toast = useToast();
  const [library, setLibrary] = useState<CompanionLibraryView | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      setLibrary(await api.listCompanions());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const importModel = useCallback(
    async (sourceDir: string): Promise<CompanionModelInfo | null> => {
      try {
        const result = await api.importCompanion({ sourceDir });
        setLibrary(result.library);
        const model = result.library.models.find((m) => m.id === result.model_id) ?? null;
        if (result.already_imported) {
          toast.warning("该伙伴已经导入");
        } else if (model) {
          toast.success(`✓ 已导入「${model.name}」`);
        }
        return model;
      } catch (e) {
        toast.error(String(e));
        return null;
      }
    },
    [toast],
  );

  const setActive = useCallback(
    async (id: string) => {
      try {
        const view = await api.setActiveCompanion({ id });
        setLibrary(view);
        const name = view.models.find((m) => m.id === id)?.name ?? id;
        toast.success(`✓ 「${name}」已设为当前使用`);
      } catch (e) {
        toast.error(String(e));
      }
    },
    [toast],
  );

  const rename = useCallback(
    async (id: string, name: string) => {
      try {
        const view = await api.renameCompanion({ id, name });
        setLibrary(view);
        toast.success(`✓ 已重命名为「${name}」`);
      } catch (e) {
        toast.error(String(e));
      }
    },
    [toast],
  );

  const remove = useCallback(
    async (id: string) => {
      const target = library?.models.find((m) => m.id === id);
      try {
        const view = await api.removeCompanion({ id });
        setLibrary(view);
        toast.success(`✓ 已移除「${target?.name ?? id}」`);
      } catch (e) {
        toast.error(String(e));
      }
    },
    [library, toast],
  );

  const saveCover = useCallback(
    async (id: string, png: number[]): Promise<CompanionLibraryView | null> => {
      try {
        const view = await api.saveCoverImage({ id, png });
        setLibrary(view);
        return view;
      } catch (e) {
        toast.error(`保存封面失败：${String(e)}`);
        return null;
      }
    },
    [toast],
  );

  return {
    library,
    loading,
    error,
    refreshing,
    refresh,
    importModel,
    setActive,
    rename,
    remove,
    saveCover,
  };
}
