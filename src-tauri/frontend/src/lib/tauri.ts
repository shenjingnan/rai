import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CatalogPage,
  CatalogQuery,
  DownloadArtifactRequest,
  DownloadTaskView,
  ModelCompatibility,
  RemoteModelDetail,
  RemoteModelFile,
  UnifiedModelItem,
} from "@/types/catalog";
import type {
  LibraryModel,
  ModelLibraryProgress,
  ModelType,
  SetCurrentResult,
  SystemResources,
} from "@/types/modelLibrary";
import type {
  AppInfo,
  AsrConfigInfo,
  AsrParamsPatch,
  AsrResult,
  CompanionLibraryView,
  ConversationRecord,
  DownloadProgress,
  ImportCompanionResult,
  KwsConfigInfo,
  KwsParamsPatch,
  KwsResult,
  ListenStopped,
  Live2dConfigInfo,
  Live2dModelInfo,
  LlmConfigInfo,
  LlmDownloadResult,
  LlmFinishReason,
  LlmParamsPatch,
  LlmStatus,
  LlmToken,
  SaveTtsVoiceRequest,
  TtsConfigInfo,
  TtsParamsPatch,
  TtsProgress,
  TtsResult,
  TtsVoice,
  VoiceError,
  VoicePlaySentence,
  VoiceReplyFinished,
  VoiceReplySentence,
  VoiceSessionStatePayload,
  VoiceStopped,
  VoiceToken,
  VoiceTranscript,
  VoiceWake,
} from "@/types/tauri";

/** 类型安全的 Tauri command 封装。 */
export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  listDevices: () => invoke<string[]>("list_devices"),
  requestMicPermission: () => invoke<boolean>("request_mic_permission"),
  getKwsConfig: () => invoke<KwsConfigInfo>("get_kws_config"),
  setKwsEnabled: (args: { enabled: boolean }) => invoke<void>("set_kws_enabled", args),
  setKwsCustomKeywords: (args: { keywords: string }) =>
    invoke<void>("set_kws_custom_keywords", args),
  setKwsParams: (args: { params: KwsParamsPatch }) => invoke<void>("set_kws_params", args),
  startListen: (args: { device: string | null; keywords: string | null }) =>
    invoke<void>("start_listen", args),
  stopListen: () => invoke<void>("stop_listen"),
  isListening: () => invoke<boolean>("is_listening"),
  downloadKwsModel: () => invoke<void>("download_kws_model"),
  getMicrophone: () => invoke<string>("get_microphone"),
  setMicrophone: (args: { mic: string }) => invoke<void>("set_microphone", args),
  getAsrConfig: () => invoke<AsrConfigInfo>("get_asr_config"),
  setAsrEnabled: (args: { enabled: boolean }) => invoke<void>("set_asr_enabled", args),
  setAsrParams: (args: { params: AsrParamsPatch }) => invoke<void>("set_asr_params", args),
  startAsrListen: (args: { device: string | null }) => invoke<void>("start_asr_listen", args),
  stopAsrListen: () => invoke<void>("stop_asr_listen"),
  isAsrListening: () => invoke<boolean>("is_asr_listening"),
  downloadAsrModel: () => invoke<void>("download_asr_model"),
  getLive2dConfig: () => invoke<Live2dConfigInfo>("get_live2d_config"),
  listCompanions: () => invoke<CompanionLibraryView>("list_companions"),
  // Tauri v2 命令参数默认 camelCase（`sourceDir` 映射 Rust 的 `source_dir`）。
  importCompanion: (args: { sourceDir: string }) =>
    invoke<ImportCompanionResult>("import_companion", args),
  setActiveCompanion: (args: { id: string }) =>
    invoke<CompanionLibraryView>("set_active_companion", args),
  renameCompanion: (args: { id: string; name: string }) =>
    invoke<CompanionLibraryView>("rename_companion", args),
  removeCompanion: (args: { id: string }) => invoke<CompanionLibraryView>("remove_companion", args),
  saveCoverImage: (args: { id: string; png: number[] }) =>
    invoke<CompanionLibraryView>("save_cover_image", args),
  getTtsConfig: () => invoke<TtsConfigInfo>("get_tts_config"),
  listTtsVoices: () => invoke<TtsVoice[]>("list_tts_voices"),
  saveTtsVoice: (args: SaveTtsVoiceRequest) => invoke<TtsVoice>("save_tts_voice", args),
  deleteTtsVoice: (args: { id: string }) => invoke<void>("delete_tts_voice", args),
  recordTtsVoice: (args: { seconds: number; device: string | null }) =>
    invoke<string>("record_tts_voice", args),
  transcribeReferenceAudio: (args: { wavPath: string }) =>
    invoke<string>("transcribe_reference_audio", args),
  synthesizeTts: (args: {
    text: string;
    speed: number | null;
    voice: string | null;
    referenceWav: string | null;
    referenceText: string | null;
  }) => invoke<void>("synthesize_tts", args),
  stopTts: () => invoke<void>("stop_tts"),
  isTtsSynthesizing: () => invoke<boolean>("is_tts_synthesizing"),
  downloadTtsModel: () => invoke<void>("download_tts_model"),
  setTtsEnabled: (args: { enabled: boolean }) => invoke<void>("set_tts_enabled", args),
  setTtsParams: (args: { params: TtsParamsPatch }) => invoke<void>("set_tts_params", args),
  setTtsVoice: (voice: string | null) => invoke<void>("set_tts_voice", { voice }),
  getLlmConfig: () => invoke<LlmConfigInfo>("get_llm_config"),
  loadLlmModel: () => invoke<void>("load_llm_model"),
  unloadLlmModel: () => invoke<void>("unload_llm_model"),
  chatLlm: (args: { text: string }) => invoke<void>("chat_llm", args),
  stopLlm: () => invoke<void>("stop_llm"),
  isLlmReady: () => invoke<boolean>("is_llm_ready"),
  setLlmModelPath: (args: { path: string }) => invoke<void>("set_llm_model_path", args),
  downloadLlmModel: (args: { id: string }) =>
    invoke<LlmDownloadResult>("download_llm_model", args),
  setLlmThinking: (args: { enabled: boolean }) => invoke<void>("set_llm_thinking", args),
  setLlmAutoLoad: (args: { enabled: boolean }) => invoke<void>("set_llm_auto_load", args),
  setLlmParams: (args: { params: LlmParamsPatch }) => invoke<void>("set_llm_params", args),
  setLlmSystemPrompt: (args: { prompt: string }) => invoke<void>("set_llm_system_prompt", args),
  // ---- 语音会话（KWS→ASR→LLM→TTS 全链路）----
  startVoiceSession: () => invoke<void>("start_voice_session"),
  stopVoiceSession: () => invoke<void>("stop_voice_session"),
  isVoiceSessionRunning: () => invoke<boolean>("is_voice_session_running"),
  // ---- 对话记录（~/.zapmomo/conversations.json）----
  getConversationRecords: () => invoke<ConversationRecord[]>("get_conversation_records"),
  clearConversationRecords: () => invoke<void>("clear_conversation_records"),
  // ---- 模型库 ----
  listModelLibrary: () => invoke<LibraryModel[]>("list_model_library"),
  getSystemResources: () => invoke<SystemResources>("get_system_resources"),
  downloadLibraryModel: (args: { id: string }) => invoke<void>("download_library_model", args),
  cancelModelDownload: () => invoke<void>("cancel_model_download"),
  setCurrentModel: (args: { id: string }) => invoke<SetCurrentResult>("set_current_model", args),
  deleteModel: (args: { id: string }) => invoke<void>("delete_model", args),
  removeLocalModel: (args: { id: string }) => invoke<void>("remove_local_model", args),
  addLocalModel: (args: {
    path: string;
    modelType?: ModelType | null;
    registryId?: string | null;
  }) => invoke<LibraryModel>("add_local_model", args),
  openModelDirectory: (args: { id: string }) => invoke<void>("open_model_directory", args),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
  // ---- 模型目录（Catalog）----
  catalogSearchModels: (provider: string, query: CatalogQuery) =>
    invoke<CatalogPage<UnifiedModelItem>>("catalog_search_models", { provider, query }),
  catalogGetModelDetail: (provider: string, modelId: string, revision?: string | null) =>
    invoke<RemoteModelDetail>("catalog_get_model_detail", { provider, modelId, revision }),
  catalogGetModelFiles: (provider: string, modelId: string, revision?: string | null) =>
    invoke<RemoteModelFile[]>("catalog_get_model_files", { provider, modelId, revision }),
  catalogGetCompatibility: (provider: string, modelId: string, revision?: string | null) =>
    invoke<ModelCompatibility>("catalog_get_compatibility", { provider, modelId, revision }),
  catalogGetModelReadme: (provider: string, modelId: string, revision?: string | null) =>
    invoke<string | null>("catalog_get_model_readme", { provider, modelId, revision }),
  // ---- 下载队列 ----
  downloadEnqueue: (request: DownloadArtifactRequest) =>
    invoke<DownloadTaskView>("download_enqueue", { request }),
  downloadCancel: (taskId: string) => invoke<void>("download_cancel", { taskId }),
  downloadSnapshot: () => invoke<DownloadTaskView[]>("download_snapshot"),
  // ---- 下载源 / token（设置页）----
  catalogGetEndpoint: () =>
    invoke<{ catalogBase: string; downloadSource: string; mirrorUrl: string }>(
      "catalog_get_endpoint",
    ),
  catalogSetEndpoint: (args: { catalogBase: string; downloadSource: string; mirrorUrl: string }) =>
    invoke<void>("catalog_set_endpoint", args),
  catalogSetToken: (token: string | null) => invoke<void>("catalog_set_token", { token }),
  saveCompanionPosition: (args: { x: number; y: number }) =>
    invoke<void>("save_companion_position", args),
  setCompanionScale: (args: { scale: number }) => invoke<void>("set_companion_scale", args),
  setCompanionOpacity: (args: { opacity: number }) => invoke<void>("set_companion_opacity", args),
  showCompanionMenu: (args: { x: number; y: number }) => invoke<void>("show_companion_menu", args),
  getHideDockIcon: () => invoke<boolean>("get_hide_dock_icon"),
  setHideDockIcon: (args: { hide: boolean }) => invoke<void>("set_hide_dock_icon", args),
  openSettings: () => invoke<void>("open_settings"),
  hideCompanion: () => invoke<void>("hide_companion"),
  quitApp: () => invoke<void>("quit_app"),
  restartApp: () => invoke<void>("restart_app"),
};

/** 类型安全的事件订阅（返回的 Promise resolve 后得到取消订阅函数）。 */
export function onKeywordDetected(handler: (result: KwsResult) => void): Promise<UnlistenFn> {
  return listen<KwsResult>("kws-detected", (e) => handler(e.payload));
}

export function onListenStopped(handler: (payload: ListenStopped) => void): Promise<UnlistenFn> {
  return listen<ListenStopped>("kws-stopped", (e) => handler(e.payload));
}

export function onListenStarted(handler: (payload: ListenStopped) => void): Promise<UnlistenFn> {
  return listen<ListenStopped>("kws-started", (e) => handler(e.payload));
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

export function onAsrStarted(handler: (payload: ListenStopped) => void): Promise<UnlistenFn> {
  return listen<ListenStopped>("asr-started", (e) => handler(e.payload));
}

export function onAsrDownloadProgress(
  handler: (payload: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>("asr-model-download-progress", (e) => handler(e.payload));
}

export function onTtsResult(handler: (result: TtsResult) => void): Promise<UnlistenFn> {
  return listen<TtsResult>("tts-result", (e) => handler(e.payload));
}

export function onTtsProgress(handler: (p: TtsProgress) => void): Promise<UnlistenFn> {
  return listen<TtsProgress>("tts-progress", (e) => handler(e.payload));
}

export function onTtsStopped(handler: (payload: ListenStopped) => void): Promise<UnlistenFn> {
  return listen<ListenStopped>("tts-stopped", (e) => handler(e.payload));
}

export function onTtsDownloadProgress(
  handler: (payload: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>("tts-model-download-progress", (e) => handler(e.payload));
}

export function onLlmDownloadProgress(
  handler: (payload: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>("llm-model-download-progress", (e) => handler(e.payload));
}

export function onLive2dModelChanged(
  handler: (info: Live2dModelInfo) => void,
): Promise<UnlistenFn> {
  return listen<Live2dModelInfo>("live2d-model-changed", (e) => handler(e.payload));
}

export function onCompanionScaleChanged(handler: (scale: number) => void): Promise<UnlistenFn> {
  return listen<number>("companion-scale-changed", (e) => handler(e.payload));
}

export function onCompanionOpacityChanged(handler: (opacity: number) => void): Promise<UnlistenFn> {
  return listen<number>("companion-opacity-changed", (e) => handler(e.payload));
}

export function onModelLibraryDownloadProgress(
  handler: (p: ModelLibraryProgress) => void,
): Promise<UnlistenFn> {
  return listen<ModelLibraryProgress>("model-library-download-progress", (e) => handler(e.payload));
}

/** 统一下载队列进度（`download-progress`；独立 taskId）。 */
export function onCatalogDownloadProgress(
  handler: (p: DownloadTaskView) => void,
): Promise<UnlistenFn> {
  return listen<DownloadTaskView>("download-progress", (e) => handler(e.payload));
}

export function onLlmToken(handler: (delta: LlmToken) => void): Promise<UnlistenFn> {
  return listen<LlmToken>("llm-token", (e) => handler(e.payload));
}

export function onLlmFinished(handler: (reason: LlmFinishReason) => void): Promise<UnlistenFn> {
  return listen<LlmFinishReason>("llm-finished", (e) => handler(e.payload));
}

export function onLlmError(handler: (error: string) => void): Promise<UnlistenFn> {
  return listen<string>("llm-error", (e) => handler(e.payload));
}

export function onLlmStatus(handler: (status: LlmStatus) => void): Promise<UnlistenFn> {
  return listen<LlmStatus>("llm-status", (e) => handler(e.payload));
}

// ---- 语音会话事件 ----

export function onVoiceSessionState(
  handler: (payload: VoiceSessionStatePayload) => void,
): Promise<UnlistenFn> {
  return listen<VoiceSessionStatePayload>("voice-session-state", (e) => handler(e.payload));
}

export function onVoiceSessionWake(handler: (payload: VoiceWake) => void): Promise<UnlistenFn> {
  return listen<VoiceWake>("voice-session-wake", (e) => handler(e.payload));
}

export function onVoiceSessionTranscript(
  handler: (payload: VoiceTranscript) => void,
): Promise<UnlistenFn> {
  return listen<VoiceTranscript>("voice-session-transcript", (e) => handler(e.payload));
}

export function onVoiceSessionToken(handler: (payload: VoiceToken) => void): Promise<UnlistenFn> {
  return listen<VoiceToken>("voice-session-token", (e) => handler(e.payload));
}

export function onVoiceSessionReply(
  handler: (payload: VoiceReplySentence) => void,
): Promise<UnlistenFn> {
  return listen<VoiceReplySentence>("voice-session-reply", (e) => handler(e.payload));
}

export function onVoiceSessionPlay(
  handler: (payload: VoicePlaySentence) => void,
): Promise<UnlistenFn> {
  return listen<VoicePlaySentence>("voice-session-play", (e) => handler(e.payload));
}

export function onVoiceSessionReplyFinished(
  handler: (payload: VoiceReplyFinished) => void,
): Promise<UnlistenFn> {
  return listen<VoiceReplyFinished>("voice-session-reply-finished", (e) => handler(e.payload));
}

export function onVoiceSessionError(handler: (payload: VoiceError) => void): Promise<UnlistenFn> {
  return listen<VoiceError>("voice-session-error", (e) => handler(e.payload));
}

export function onVoiceSessionStopped(
  handler: (payload: VoiceStopped) => void,
): Promise<UnlistenFn> {
  return listen<VoiceStopped>("voice-session-stopped", (e) => handler(e.payload));
}

/**
 * 把本地绝对路径转成 Tauri asset 协议 URL，供 Live2D 运行时加载。
 *
 * 不能直接用 `@tauri-apps/api/core` 的 `convertFileSrc`：它用 `encodeURIComponent`
 * 编码整个路径（含 `/`），导致 URL 的 path 退化成单个段、没有目录结构，Live2D
 * 运行时解析模型清单里的相对路径（如 `xxx.moc3`）时会错误地解析到根目录。
 *
 * 这里改为逐段编码、保留 `/` 分隔符——Tauri 的 asset handler 会「skip leading /」，
 * 去掉一个 `/` 后得到的仍是绝对路径（如 `/Users/...`），从而同时满足
 * 「相对路径正确解析」与「文件正确打开」两个要求。
 *
 * 平台差异（同 convertFileSrc 的规则）：
 * - Windows 的 WebView2 是 Chromium 内核，禁止对自定义 scheme 发跨源请求，
 *   必须用虚拟主机形式 `http://asset.localhost/<path>`（CSP 已放行该来源）；
 * - macOS/Linux 保持 `asset://localhost/<path>`。
 *
 * 另外 Tauri 返回的是原生路径：Windows 用 `\` 分隔，需先归一化为 `/`，
 * 否则整条路径会被编码成单个段（`%5C`），相对资源解析会全部失配。
 */
export function toAssetUrl(path: string): string {
  const isWindows = navigator.userAgent.includes("Windows");
  const segments = path
    .replace(/\\/g, "/")
    .split("/")
    .map((s) => encodeURIComponent(s))
    .join("/");
  return isWindows ? `http://asset.localhost/${segments}` : `asset://localhost/${segments}`;
}
