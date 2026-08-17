import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "@/components/ui/toast";
import { api, onModelLibraryDownloadProgress } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";
import type {
  LibraryModel,
  LibraryProgressStage,
  ModelLibraryProgress,
  ModelType,
  SetCurrentResult,
  SystemResources,
} from "@/types/modelLibrary";

export interface ModelLibraryState {
  models: LibraryModel[] | null;
  loading: boolean;
  error: string | null;
  refreshing: boolean;
  refresh: () => Promise<void>;

  downloadingId: string | null;
  progress: ModelLibraryProgress | null;
  download: (id: string) => Promise<void>;
  cancelDownload: () => Promise<void>;

  /** 设为当前模型（后端事务 + 返回消息；同步刷新模型库与「模型与能力」页） */
  setCurrent: (id: string) => Promise<void>;
  /** 卸载（managed 删文件）/ 移除（external 只取消注册） */
  remove: (id: string) => Promise<void>;
  addLocal: (
    path: string,
    modelType?: ModelType | null,
    registryId?: string | null,
  ) => Promise<void>;

  resources: SystemResources | null;
  resourcesLoading: boolean;
  refreshResources: () => Promise<void>;
}

/** 模型库全局状态：列表快照 + 下载进度 + 切换/删除/导入。 */
export function useModelLibrary(): ModelLibraryState {
  const { kws, asr, llm, tts } = useRuntime();
  const toast = useToast();

  const [models, setModels] = useState<LibraryModel[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [progress, setProgress] = useState<ModelLibraryProgress | null>(null);
  const [resources, setResources] = useState<SystemResources | null>(null);
  const [resourcesLoading, setResourcesLoading] = useState(false);

  // 记录下载的终止阶段（done / cancelled），供 download() 决定 Toast 文案
  const terminalStage = useRef<LibraryProgressStage | null>(null);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      setModels(await api.listModelLibrary());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  const refreshResources = useCallback(async () => {
    setResourcesLoading(true);
    try {
      setResources(await api.getSystemResources());
    } catch (e) {
      toast.error(`资源检测失败：${String(e)}`);
    } finally {
      setResourcesLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const unsubs = [
      onModelLibraryDownloadProgress((p) => {
        setProgress(p);
        if (p.stage === "done" || p.stage === "cancelled" || p.stage === "failed") {
          terminalStage.current = p.stage;
        }
      }),
    ];
    return () => {
      unsubs.forEach((u) => {
        u.then((fn) => fn());
      });
    };
  }, []);

  const displayName = useCallback(
    (id: string) => models?.find((m) => m.id === id)?.displayName ?? id,
    [models],
  );

  const download = useCallback(
    async (id: string) => {
      setDownloadingId(id);
      setProgress(null);
      terminalStage.current = null;
      try {
        await api.downloadLibraryModel({ id });
        const stage = terminalStage.current;
        if (stage === "cancelled") {
          toast.warning("已取消下载");
        } else if (stage === "done") {
          toast.success(`✓ ${displayName(id)} 下载完成`);
        } else {
          toast.success(`✓ ${displayName(id)} 下载完成`);
        }
      } catch (e) {
        toast.error(`模型下载失败：${String(e)}`);
      } finally {
        setDownloadingId(null);
        setProgress(null);
        terminalStage.current = null;
        await refresh();
      }
    },
    [toast, displayName, refresh],
  );

  const cancelDownload = useCallback(async () => {
    try {
      await api.cancelModelDownload();
    } catch (e) {
      toast.error(String(e));
    }
  }, [toast]);

  const setCurrent = useCallback(
    async (id: string) => {
      let res: SetCurrentResult;
      try {
        res = await api.setCurrentModel({ id });
      } catch (e) {
        toast.error(String(e));
        return;
      }
      // 1) 刷新模型库快照（current badge / runtime status）
      await refresh();
      // 2) 同步「模型与能力」页与各配置页（只读刷新，不影响运行时）
      await Promise.allSettled([
        kws.config.refresh(),
        asr.config.refresh(),
        llm.refreshConfig(),
        tts.refreshConfig(),
      ]);
      // 3) 按后端返回结果 Toast（不在前端猜 runtime 行为）
      toast.success(res.message);
    },
    [toast, refresh, kws, asr, llm, tts],
  );

  const remove = useCallback(
    async (id: string) => {
      const target = models?.find((m) => m.id === id);
      try {
        await api.deleteModel({ id });
        toast.success(
          target?.ownership === "external" ? "✓ 已从模型库移除，不会删除原始文件" : "✓ 模型已卸载",
        );
      } catch (e) {
        toast.error(String(e));
      } finally {
        await refresh();
      }
    },
    [models, toast, refresh],
  );

  const addLocal = useCallback(
    async (path: string, modelType?: ModelType | null, registryId?: string | null) => {
      try {
        const added = await api.addLocalModel({ path, modelType, registryId });
        toast.success(`✓ 已添加本地模型：${added.displayName}`);
        await refresh();
      } catch (e) {
        toast.error(String(e));
      }
    },
    [toast, refresh],
  );

  return {
    models,
    loading,
    error,
    refreshing,
    refresh,
    downloadingId,
    progress,
    download,
    cancelDownload,
    setCurrent,
    remove,
    addLocal,
    resources,
    resourcesLoading,
    refreshResources,
  };
}
