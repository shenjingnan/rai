import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  AsrConfigInfo,
  AsrResult,
  DownloadProgress,
  KwsConfigInfo,
  KwsResult,
  ListenStopped,
  Live2dConfigInfo,
  Live2dModelInfo,
} from "@/types/tauri";

/** 类型安全的 Tauri command 封装。 */
export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  listDevices: () => invoke<string[]>("list_devices"),
  getKwsConfig: () => invoke<KwsConfigInfo>("get_kws_config"),
  startListen: (args: { device: string | null; keywords: string | null }) =>
    invoke<void>("start_listen", args),
  stopListen: () => invoke<void>("stop_listen"),
  isListening: () => invoke<boolean>("is_listening"),
  downloadKwsModel: () => invoke<void>("download_kws_model"),
  getAsrConfig: () => invoke<AsrConfigInfo>("get_asr_config"),
  startAsrListen: (args: { device: string | null }) => invoke<void>("start_asr_listen", args),
  stopAsrListen: () => invoke<void>("stop_asr_listen"),
  isAsrListening: () => invoke<boolean>("is_asr_listening"),
  downloadAsrModel: () => invoke<void>("download_asr_model"),
  getLive2dConfig: () => invoke<Live2dConfigInfo>("get_live2d_config"),
  setLive2dModel: (args: { dir: string }) => invoke<Live2dModelInfo>("set_live2d_model", args),
};

/** 类型安全的事件订阅（返回的 Promise resolve 后得到取消订阅函数）。 */
export function onKeywordDetected(handler: (result: KwsResult) => void): Promise<UnlistenFn> {
  return listen<KwsResult>("kws-detected", (e) => handler(e.payload));
}

export function onListenStopped(handler: (payload: ListenStopped) => void): Promise<UnlistenFn> {
  return listen<ListenStopped>("kws-stopped", (e) => handler(e.payload));
}

export function onDownloadProgress(
  handler: (payload: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>("kws-model-download-progress", (e) => handler(e.payload));
}

export function onAsrResult(handler: (result: AsrResult) => void): Promise<UnlistenFn> {
  return listen<AsrResult>("asr-result", (e) => handler(e.payload));
}

export function onAsrStopped(handler: (payload: ListenStopped) => void): Promise<UnlistenFn> {
  return listen<ListenStopped>("asr-stopped", (e) => handler(e.payload));
}

export function onAsrDownloadProgress(
  handler: (payload: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>("asr-model-download-progress", (e) => handler(e.payload));
}
