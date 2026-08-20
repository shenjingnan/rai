import { useCallback, useEffect, useRef, useState } from "react";
import type { LlmState } from "@/hooks/useLlm";
import type { VoiceSessionState } from "@/hooks/useVoiceSession";
import { api, onLlmDownloadProgress } from "@/lib/tauri";
import type { DownloadProgress } from "@/types/tauri";

export interface LlmModelDownloadState {
  downloading: boolean;
  /** 正在下载的 registry 模型 id（null = 空闲） */
  currentId: string | null;
  progress: DownloadProgress | null;
  error: string | null;
  download: (id: string) => Promise<void>;
}

/**
 * LLM 预设模型下载：订阅 `llm-model-download-progress`，`download(id)` 下载 registry 预设。
 * 完成后刷新配置（models_present → true，下载区消失、加载开关解锁）；在后端写入了配置
 * （applied）且无 voice 会话、引擎未加载时自动加载，让新装用户下载完即可直接测试。
 * voice 运行中不自动加载：load_llm_impl 无切换保护，替换引擎会造成 voice 双模型。
 */
export function useLlmModelDownload(
  llm: Pick<LlmState, "ready" | "load" | "refreshConfig">,
  voice: Pick<VoiceSessionState, "running">,
): LlmModelDownloadState {
  const [downloading, setDownloading] = useState(false);
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  // 下载是长 await，ready/running 需读最新值（闭包快照会漏掉下载期间的状态变化）
  const llmRef = useRef(llm);
  llmRef.current = llm;
  const voiceRef = useRef(voice);
  voiceRef.current = voice;

  useEffect(() => {
    const unlisten = onLlmDownloadProgress(setProgress);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const download = useCallback(async (id: string) => {
    setDownloading(true);
    setCurrentId(id);
    setError(null);
    setProgress(null);
    try {
      const r = await api.downloadLlmModel({ id });
      await llmRef.current.refreshConfig();
      if (r.applied && !llmRef.current.ready && !voiceRef.current.running) {
        void llmRef.current.load();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setDownloading(false);
      setCurrentId(null);
    }
  }, []);

  return { downloading, currentId, progress, error, download };
}
