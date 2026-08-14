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
