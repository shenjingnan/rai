import { useEffect, useState } from "react";
import { api, onListenStopped } from "@/lib/tauri";

export interface ListeningState {
  isListening: boolean;
  error: string | null;
  start: (device: string | null, keywords: string | null) => Promise<void>;
  stop: () => Promise<void>;
}

/**
 * 监听状态管理：初始化时回读后端状态，订阅 `kws-stopped` 事件；
 * start/stop 包装对应 command 并同步 UI 状态与错误。
 */
export function useListening(): ListeningState {
  const [isListening, setIsListening] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .isListening()
      .then(setIsListening)
      .catch(() => {});

    const unlisten = onListenStopped((payload) => {
      setIsListening(false);
      if (payload.error) setError(payload.error);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const start = async (device: string | null, keywords: string | null) => {
    setError(null);
    try {
      await api.startListen({ device, keywords });
      setIsListening(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const stop = async () => {
    try {
      await api.stopListen();
      setIsListening(false);
    } catch (e) {
      setError(String(e));
    }
  };

  return { isListening, error, start, stop };
}
