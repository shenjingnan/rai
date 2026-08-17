// Tauri 后端命令 / 事件的类型契约。
// 与 src-tauri/src/lib.rs 的命令签名及 src/kws/reaction.rs 的 KwsResult 一一对应。

/** `get_app_info` 返回 */
export interface AppInfo {
  version: string;
  product_name: string;
}

/** `get_kws_config` 返回 */
export interface KwsConfigInfo {
  /** 是否启用 KWS（打开开关即持久化；下次启动自动监听的前提） */
  enabled: boolean;
  /** 持久化的会话级自定义唤醒词（原始字符串，多个用 / 分隔；空 = 模型内置） */
  custom_keywords: string;
  model_dir: string;
  provider: string;
  num_threads: number;
  sample_rate: number;
  /** 每次喂给模型的采样数（@16k） */
  chunk_size: number;
  /** 关键词 boosting 分数 */
  keywords_score: number;
  /** 触发阈值（灵敏度，0~1） */
  keywords_threshold: number;
  debug: boolean;
  keywords: string[];
  models_present: boolean;
  model_downloading: boolean;
  settings_path: string;
}

/** `set_kws_params` 载荷：可调整的 KWS 引擎/运行参数（snake_case 直传，缺省项不修改）。 */
export interface KwsParamsPatch {
  keywords_threshold?: number;
  keywords_score?: number;
  chunk_size?: number;
  num_threads?: number;
  debug?: boolean;
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

/** 解析后的 LLM 采样/引擎参数（对应后端 GenParams，snake_case 直传）。 */
export interface LlmParams {
  context_size: number;
  batch_size: number;
  max_tokens: number;
  temperature: number;
  top_p: number;
  top_k: number;
  min_p: number;
  repeat_penalty: number;
  seed: number;
  threads: number;
  gpu_layers: number;
  enable_thinking: boolean;
}

/** `set_llm_params` 载荷：11 项采样/引擎参数（enable_thinking 走独立命令，不入批）。 */
export type LlmParamsPatch = Omit<LlmParams, "enable_thinking">;

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
  /** 当前生效的角色 system prompt */
  system_prompt: string;
  /** 当前生效的采样/引擎参数（已 resolve） */
  params: LlmParams;
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
