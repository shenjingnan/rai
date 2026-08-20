import { useCallback, useEffect, useState } from "react";
import { useToast } from "@/components/ui/toast";
import { api } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";
import type { LibraryModel, SetCurrentResult } from "@/types/modelLibrary";

/** LLM 配置页推荐的精简预设（每系列一个推荐量化；id = models/model_registry.json 的 registry id）。 */
export const LLM_PRESETS = [
  {
    id: "qwen3-0.6b-q4-k-m",
    name: "Qwen3 0.6B",
    tagline: "轻量 · 适合入门设备",
    sizeBytes: 396_705_472,
  },
  {
    id: "qwen3-1.7b-q4-k-m",
    name: "Qwen3 1.7B",
    tagline: "均衡 · 轻量硬件",
    sizeBytes: 1_107_409_472,
  },
  {
    id: "qwen3-4b-instruct-2507-q4-k-m",
    name: "Qwen3 4B",
    tagline: "推荐 · 质量与速度平衡",
    sizeBytes: 2_497_281_120,
  },
  {
    id: "qwen3-8b-q4-k-m",
    name: "Qwen3 8B",
    tagline: "更强 · 需要较强硬件",
    sizeBytes: 5_027_784_512,
  },
  {
    id: "llama-3.2-1b-instruct-q4-k-m",
    name: "Llama 3.2 1B",
    tagline: "轻量 · 英文场景",
    sizeBytes: 807_694_368,
  },
] as const;

export interface LlmPresetsState {
  /** `list_model_library` 快照（含预设的安装 / current 状态）；null = 尚未加载 */
  models: LibraryModel[] | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  /** 下载预设：复用全局一键下载链路（完成后自动写配置并加载） */
  download: (id: string) => Promise<void>;
  /** 设为当前模型 */
  setCurrent: (id: string) => Promise<void>;
  /** 卸载（managed 删文件）/ 移除（external 只取消注册） */
  remove: (id: string) => Promise<void>;
}

/**
 * LLM 预设模型选择状态：从模型库列表过滤出预设，提供下载 / 设为当前 / 卸载。
 * 数据用 `list_model_library`（同一后端真相源，含 install_state + current）。
 */
export function useLlmPresets(): LlmPresetsState {
  const { llm } = useRuntime();
  const toast = useToast();
  const [models, setModels] = useState<LibraryModel[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setModels(await api.listModelLibrary());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const download = useCallback(
    async (id: string) => {
      try {
        await llm.download.download(id);
      } finally {
        await refresh();
      }
    },
    [llm.download, refresh],
  );

  const setCurrent = useCallback(
    async (id: string) => {
      let res: SetCurrentResult;
      try {
        res = await api.setCurrentModel({ id });
      } catch (e) {
        toast.error(String(e));
        return;
      }
      toast.success(res.message);
      await Promise.allSettled([llm.refreshConfig(), refresh()]);
    },
    [toast, llm.refreshConfig, refresh],
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
        return;
      }
      await Promise.allSettled([llm.refreshConfig(), refresh()]);
    },
    [models, toast, llm.refreshConfig, refresh],
  );

  return { models, loading, error, refresh, download, setCurrent, remove };
}
