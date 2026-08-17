import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { AsrConfigInfo, AsrParamsPatch } from "@/types/tauri";

export interface AsrConfigState {
  config: AsrConfigInfo | null;
  error: string | null;
  refresh: () => Promise<void>;
  /** 持久化 ASR 引擎/运行参数（[asr] 批写入），写成功后回读配置。 */
  setParams: (patch: AsrParamsPatch) => Promise<void>;
}

/** 读取 ASR 配置与模型状态。 */
export function useAsrConfig(): AsrConfigState {
  const [config, setConfig] = useState<AsrConfigInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setConfig(await api.getAsrConfig());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const setParams = useCallback(
    async (patch: AsrParamsPatch) => {
      // 保存失败向上抛出，由调用方（高级参数表单）展示内联错误
      await api.setAsrParams({ params: patch });
      await refresh();
    },
    [refresh],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { config, error, refresh, setParams };
}
