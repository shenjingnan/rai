import { type RefObject, useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  onTtsDownloadProgress,
  onTtsProgress,
  onTtsResult,
  onTtsStopped,
  toAssetUrl,
} from "@/lib/tauri";
import type { DownloadProgress, TtsConfigInfo, TtsResult, TtsVoice } from "@/types/tauri";

/** 自定义音色在 `selectedVoice` 中的哨兵值。 */
export const CUSTOM_VOICE = "__custom__";

export interface TtsState {
  config: TtsConfigInfo | null;
  configError: string | null;
  refreshConfig: () => Promise<void>;
  voices: TtsVoice[];
  selectedVoice: string;
  setSelectedVoice: (id: string) => void;
  customWav: string | null;
  setCustomWav: (path: string | null) => void;
  customText: string | null;
  setCustomText: (text: string | null) => void;
  transcribing: boolean;
  transcribeError: string | null;
  transcribe: () => Promise<void>;
  synthesizing: boolean;
  progress: number | null;
  result: TtsResult | null;
  error: string | null;
  synthesize: (text: string) => Promise<void>;
  stop: () => Promise<void>;
  downloading: boolean;
  downloadProgress: DownloadProgress | null;
  downloadError: string | null;
  download: () => Promise<void>;
  audioUrl: string | null;
  audioRef: RefObject<HTMLAudioElement | null>;
  play: () => void;
}

/**
 * TTS 状态管理：配置读取、音色列表、合成触发/进度/结果、模型下载、播放。
 * 合成走后台线程，进度与结果经 `tts-progress` / `tts-result` / `tts-stopped` 事件同步。
 */
export function useTts(): TtsState {
  const [config, setConfig] = useState<TtsConfigInfo | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);
  const [voices, setVoices] = useState<TtsVoice[]>([]);
  const [selectedVoice, setSelectedVoice] = useState("");
  const [customWav, setCustomWav] = useState<string | null>(null);
  const [customText, setCustomText] = useState<string | null>(null);
  const [transcribing, setTranscribing] = useState(false);
  const [transcribeError, setTranscribeError] = useState<string | null>(null);
  const [synthesizing, setSynthesizing] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [result, setResult] = useState<TtsResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const refreshConfig = useCallback(async () => {
    try {
      setConfig(await api.getTtsConfig());
      setConfigError(null);
    } catch (e) {
      setConfigError(String(e));
    }
  }, []);

  useEffect(() => {
    void refreshConfig();
  }, [refreshConfig]);

  // 加载内置音色列表（模型下载后可用；未下载时为空）。
  useEffect(() => {
    api
      .listTtsVoices()
      .then(setVoices)
      .catch(() => setVoices([]));
  }, []);

  useEffect(() => {
    const unsubs = [
      onTtsProgress((p) => setProgress(p.percent)),
      onTtsResult((r) => setResult(r)),
      onTtsStopped((payload) => {
        setSynthesizing(false);
        if (payload.error) setError(payload.error);
      }),
      onTtsDownloadProgress(setDownloadProgress),
    ];
    return () => {
      unsubs.forEach((u) => {
        u.then((fn) => fn());
      });
    };
  }, []);

  const synthesize = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (!trimmed) return;
      setError(null);
      setResult(null);
      setProgress(null);
      setSynthesizing(true);
      const isCustom = selectedVoice === CUSTOM_VOICE;
      try {
        await api.synthesizeTts({
          text: trimmed,
          speed: null,
          voice: isCustom ? null : selectedVoice || null,
          referenceWav: isCustom ? customWav : null,
          referenceText: isCustom ? customText : null,
        });
      } catch (e) {
        setError(String(e));
        setSynthesizing(false);
      }
    },
    [selectedVoice, customWav, customText],
  );

  // 用 ASR 离线转写参考音频，自动填充「逐字转写文本」（用户可再手动修正）。
  const transcribe = useCallback(async () => {
    if (!customWav) return;
    setTranscribing(true);
    setTranscribeError(null);
    try {
      const text = await api.transcribeReferenceAudio({ wavPath: customWav });
      setCustomText(text);
    } catch (e) {
      setTranscribeError(String(e));
    } finally {
      setTranscribing(false);
    }
  }, [customWav]);

  const stop = useCallback(async () => {
    try {
      await api.stopTts();
      setSynthesizing(false);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const download = useCallback(async () => {
    setDownloading(true);
    setDownloadError(null);
    setDownloadProgress(null);
    try {
      await api.downloadTtsModel();
      void refreshConfig();
      const v = await api.listTtsVoices();
      setVoices(v);
    } catch (e) {
      setDownloadError(String(e));
    } finally {
      setDownloading(false);
    }
  }, [refreshConfig]);

  const play = useCallback(() => {
    const el = audioRef.current;
    if (!el) return;
    void el.play().catch(() => {});
  }, []);

  const audioUrl = result ? toAssetUrl(result.path) : null;

  return {
    config,
    configError,
    refreshConfig,
    voices,
    selectedVoice,
    setSelectedVoice,
    customWav,
    setCustomWav,
    customText,
    setCustomText,
    transcribing,
    transcribeError,
    transcribe,
    synthesizing,
    progress,
    result,
    error,
    synthesize,
    stop,
    downloading,
    downloadProgress,
    downloadError,
    download,
    audioUrl,
    audioRef,
    play,
  };
}
