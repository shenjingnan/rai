import { useCallback, useEffect, useState } from "react";
import { api, onLlmError, onLlmFinished, onLlmStatus, onLlmToken } from "@/lib/tauri";
import type { LlmConfigInfo } from "@/types/tauri";

export interface LlmState {
  config: LlmConfigInfo | null;
  configError: string | null;
  refreshConfig: () => Promise<void>;
  ready: boolean;
  loading: boolean;
  generating: boolean;
  response: string;
  error: string | null;
  load: () => Promise<void>;
  unload: () => Promise<void>;
  chat: (text: string) => Promise<void>;
  stop: () => Promise<void>;
  setThinking: (enabled: boolean) => Promise<void>;
}

/**
 * LLM 状态管理：配置读取、模型加载/卸载、流式对话。
 * 加载结果经 `llm-status`/`llm-error` 事件同步，token 流经 `llm-token`，
 * 生成结束经 `llm-finished`。
 */
export function useLlm(): LlmState {
  const [config, setConfig] = useState<LlmConfigInfo | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [loading, setLoading] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [response, setResponse] = useState("");
  const [error, setError] = useState<string | null>(null);

  const refreshConfig = useCallback(async () => {
    try {
      const c = await api.getLlmConfig();
      setConfig(c);
      setReady(c.ready);
      setConfigError(null);
    } catch (e) {
      setConfigError(String(e));
    }
  }, []);

  useEffect(() => {
    void refreshConfig();
  }, [refreshConfig]);

  useEffect(() => {
    const unsubs = [
      onLlmToken((delta) => setResponse((prev) => prev + delta.text)),
      onLlmFinished(() => setGenerating(false)),
      onLlmError((e) => {
        setError(e);
        setLoading(false);
        setGenerating(false);
      }),
      onLlmStatus((s) => {
        setReady(s.ready);
        setLoading(false);
      }),
    ];
    return () => {
      unsubs.forEach((u) => {
        u.then((fn) => fn());
      });
    };
  }, []);

  const load = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      await api.loadLlmModel();
      // 加载结果经 llm-status / llm-error 事件更新
    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  }, []);

  const unload = useCallback(async () => {
    try {
      await api.unloadLlmModel();
      setReady(false);
      setResponse("");
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const chat = useCallback(async (text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    setError(null);
    setResponse("");
    setGenerating(true);
    try {
      await api.chatLlm({ text: trimmed });
      // token 流经 llm-token 事件，结束经 llm-finished
    } catch (e) {
      setError(String(e));
      setGenerating(false);
    }
  }, []);

  const stop = useCallback(async () => {
    try {
      await api.stopLlm();
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const setThinking = useCallback(
    async (enabled: boolean) => {
      try {
        await api.setLlmThinking({ enabled });
        await refreshConfig();
      } catch (e) {
        setError(String(e));
      }
    },
    [refreshConfig],
  );

  return {
    config,
    configError,
    refreshConfig,
    ready,
    loading,
    generating,
    response,
    error,
    load,
    unload,
    chat,
    stop,
    setThinking,
  };
}
