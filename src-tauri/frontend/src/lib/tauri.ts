import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  DownloadProgress,
  KwsConfigInfo,
  KwsResult,
  ListenStopped,
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
