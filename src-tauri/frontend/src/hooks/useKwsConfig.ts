import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { KwsConfigInfo } from "@/types/tauri";

export interface KwsConfigState {
  config: KwsConfigInfo | null;
  error: string | null;
  refresh: () => Promise<void>;
}

/** 读取 KWS 配置与模型状态。 */
export function useKwsConfig(): KwsConfigState {
  const [config, setConfig] = useState<KwsConfigInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setConfig(await api.getKwsConfig());
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
