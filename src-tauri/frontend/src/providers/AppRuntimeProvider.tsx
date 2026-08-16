import { type ReactNode, useState } from "react";
import { useAppInfo } from "@/hooks/useAppInfo";
import { useAsrConfig } from "@/hooks/useAsrConfig";
import { useAsrListening } from "@/hooks/useAsrListening";
import { useAsrModelDownload } from "@/hooks/useAsrModelDownload";
import { useAsrResults } from "@/hooks/useAsrResults";
import { useDevices } from "@/hooks/useDevices";
import { useKwsConfig } from "@/hooks/useKwsConfig";
import { useListening } from "@/hooks/useListening";
import { useLive2dConfig } from "@/hooks/useLive2dConfig";
import { useLive2dModel } from "@/hooks/useLive2dModel";
import { useLlm } from "@/hooks/useLlm";
import { useModelDownload } from "@/hooks/useModelDownload";
import { useResults } from "@/hooks/useResults";
import { useTts } from "@/hooks/useTts";
import { RuntimeContext, type RuntimeState } from "./RuntimeContext";

/**
 * 运行态 Provider：把 KWS / ASR / LLM / TTS / Live2D 的 hooks 集中在此调用，
 * 并常驻于路由外层（`<Routes>` 之外），使监听/下载/流式/加载状态不随页面切换丢失。
 * Router 只负责「当前显示哪个 UI」，不决定 runtime 生命周期。
 */
export function AppRuntimeProvider({ children }: { children: ReactNode }) {
  const appInfo = useAppInfo();
  const devices = useDevices();
  const kwsConfig = useKwsConfig();
  const kwsDownload = useModelDownload(kwsConfig.refresh);
  const listening = useListening();
  const results = useResults();
  const asrConfig = useAsrConfig();
  const asrDownload = useAsrModelDownload(asrConfig.refresh);
  const asrListening = useAsrListening();
  const asrResults = useAsrResults();
  const llm = useLlm();
  const tts = useTts();
  const live2dConfig = useLive2dConfig();
  const live2dModel = useLive2dModel(live2dConfig.refresh);
  const [device, setDevice] = useState("");

  const anyListening = listening.isListening || asrListening.isListening;

  const value: RuntimeState = {
    appInfo,
    devices,
    kws: { config: kwsConfig, download: kwsDownload, listening, results },
    asr: {
      config: asrConfig,
      download: asrDownload,
      listening: asrListening,
      results: asrResults,
    },
    llm,
    tts,
    live2d: { config: live2dConfig, model: live2dModel },
    device,
    setDevice,
    anyListening,
  };

  return <RuntimeContext.Provider value={value}>{children}</RuntimeContext.Provider>;
}
