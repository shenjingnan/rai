import { useCallback, useEffect, useState } from "react";
import { api, onLive2dModelChanged } from "@/lib/tauri";
import type { Live2dConfigInfo } from "@/types/tauri";

export interface Live2dConfigState {
  config: Live2dConfigInfo | null;
  error: string | null;
  refresh: () => Promise<void>;
}

/** 读取 Live2D 配置与模型状态，并在模型变更时自动刷新。 */
export function useLive2dConfig(): Live2dConfigState {
  const [config, setConfig] = useState<Live2dConfigInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setConfig(await api.getLive2dConfig());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    // 订阅模型变更事件，桌宠窗口无需重启即可刷新角色
    const unlisten = onLive2dModelChanged(() => {
      void refresh();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  return { config, error, refresh };
}
