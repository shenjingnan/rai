// Tauri 后端命令 / 事件的类型契约。
// 与 src-tauri/src/lib.rs 的命令签名及 src/kws/reaction.rs 的 KwsResult 一一对应。

/** `get_app_info` 返回 */
export interface AppInfo {
  version: string;
  product_name: string;
}

/** `get_kws_config` 返回 */
export interface KwsConfigInfo {
  model_dir: string;
  provider: string;
  num_threads: number;
  sample_rate: number;
  keywords: string[];
  models_present: boolean;
  model_downloading: boolean;
  settings_path: string;
}

/** `kws-detected` 事件载荷（对应后端 KwsResult） */
export interface KwsResult {
  keyword: string;
  tokens: string;
  tokens_arr: string[];
  timestamps: number[];
  start_time: number;
  json: string;
}

/** `kws-stopped` 事件载荷（正常停止时 error 为 null） */
export interface ListenStopped {
  error: string | null;
}

/** `kws-model-download-progress` / `asr-model-download-progress` 事件载荷 */
export type DownloadStage = "downloading" | "verifying" | "extracting" | "done";

export interface DownloadProgress {
  stage: DownloadStage;
  percent: number;
  message: string;
}

/** `get_asr_config` 返回 */
export interface AsrConfigInfo {
  model_dir: string;
  provider: string;
  num_threads: number;
  sample_rate: number;
  models_present: boolean;
  punctuation_present: boolean;
  model_downloading: boolean;
  settings_path: string;
}

/** `asr-result` 事件载荷（对应后端 AsrResult） */
export interface AsrResult {
  text: string;
  tokens: string[];
  timestamps: number[] | null;
  start_time: number | null;
  is_final: boolean;
}

/** `get_live2d_config` 返回 */
export interface Live2dConfigInfo {
  model_dir: string | null;
  model_file: string | null;
  format: string | null;
  models_present: boolean;
  window_scale: number | null;
  settings_path: string;
}

/** `set_live2d_model` 返回 */
export interface Live2dModelInfo {
  model_dir: string;
  model_file: string;
  format: string;
}

/** `get_tts_config` 返回 */
export interface TtsConfigInfo {
  model_dir: string;
  provider: string;
  num_threads: number;
  enabled: boolean;
  models_present: boolean;
  model_downloading: boolean;
  settings_path: string;
}

/** `tts-result` 事件载荷（对应后端 TtsResult） */
export interface TtsResult {
  path: string;
  duration: number;
  sample_rate: number;
}

/** `tts-progress` 事件载荷（对应后端 TtsProgress） */
export interface TtsProgress {
  percent: number;
}

/** `list_tts_voices` 返回的音色（对应后端 TtsVoice） */
export interface TtsVoice {
  id: string;
  name: string;
  wav_path: string;
  reference_text: string;
}

/** `get_llm_config` 返回 */
export interface LlmConfigInfo {
  enabled: boolean;
  provider: string;
  model_path: string;
  models_present: boolean;
  ready: boolean;
  enable_thinking: boolean;
  auto_load: boolean;
  settings_path: string;
}

/** `llm-token` 事件载荷（对应后端 TokenDelta） */
export interface LlmToken {
  text: string;
  is_final: boolean;
}

/** `llm-finished` 事件载荷（对应后端 FinishReason，序列化为小写） */
export type LlmFinishReason = "eos" | "max_tokens" | "cancelled" | "error";

/** `llm-status` 事件载荷（对应后端 LlmStatusPayload） */
export interface LlmStatus {
  ready: boolean;
}
