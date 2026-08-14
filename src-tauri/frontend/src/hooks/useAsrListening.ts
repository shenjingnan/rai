import { useEffect, useState } from "react";
import { api, onAsrStopped } from "@/lib/tauri";

export interface AsrListeningState {
  isListening: boolean;
  error: string | null;
  start: (device: string | null) => Promise<void>;
  stop: () => Promise<void>;
}

/**
 * ASR 识别状态管理：初始化时回读后端状态，订阅 `asr-stopped` 事件；
 * start/stop 包装对应 command 并同步 UI 状态与错误。
 */
export function useAsrListening(): AsrListeningState {
  const [isListening, setIsListening] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .isAsrListening()
      .then(setIsListening)
      .catch(() => {});

    const unlisten = onAsrStopped((payload) => {
      setIsListening(false);
      if (payload.error) setError(payload.error);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const start = async (device: string | null) => {
    setError(null);
    try {
      await api.startAsrListen({ device });
      setIsListening(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const stop = async () => {
    try {
      await api.stopAsrListen();
      setIsListening(false);
    } catch (e) {
      setError(String(e));
    }
  };

  return { isListening, error, start, stop };
}
