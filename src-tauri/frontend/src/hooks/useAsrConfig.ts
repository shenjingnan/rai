import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { AsrConfigInfo } from "@/types/tauri";

export interface AsrConfigState {
  config: AsrConfigInfo | null;
  error: string | null;
  refresh: () => Promise<void>;
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

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { config, error, refresh };
}
