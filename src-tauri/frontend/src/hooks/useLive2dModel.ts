import { useCallback, useState } from "react";
import { api, toAssetUrl } from "@/lib/tauri";
import type { Live2dModelInfo } from "@/types/tauri";

export interface Live2dModelState {
  /** 已加载模型的 asset:// URL（由 setLive2dModel 成功后产生） */
  modelUrl: string | null;
  modelInfo: Live2dModelInfo | null;
  loading: boolean;
  error: string | null;
  load: (dir: string) => Promise<void>;
  clear: () => void;
}

/** 选择并加载本地 Live2D 模型目录。 */
export function useLive2dModel(onSuccess?: () => void): Live2dModelState {
  const [modelUrl, setModelUrl] = useState<string | null>(null);
  const [modelInfo, setModelInfo] = useState<Live2dModelInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    async (dir: string) => {
      setLoading(true);
      setError(null);
      try {
        const info = await api.setLive2dModel({ dir });
        setModelInfo(info);
        setModelUrl(toAssetUrl(info.model_file));
        onSuccess?.();
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    },
    [onSuccess],
  );

  const clear = useCallback(() => {
    setModelUrl(null);
    setModelInfo(null);
  }, []);

  return { modelUrl, modelInfo, loading, error, load, clear };
}
