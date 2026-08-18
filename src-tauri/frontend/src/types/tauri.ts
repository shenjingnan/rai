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

/** `get_asr_config` 返回（含可经 `set_asr_params` 调整的引擎参数） */
export interface AsrConfigInfo {
  enabled: boolean;
  model_dir: string;
  provider: string;
  num_threads: number;
  sample_rate: number;
  chunk_size: number;
  decoding_method: string;
  enable_endpoint: boolean;
  rule1_min_trailing_silence: number;
  rule2_min_trailing_silence: number;
  rule3_min_utterance_length: number;
  blank_penalty: number;
  hotwords: string | null;
  enable_punctuation: boolean;
  debug: boolean;
  models_present: boolean;
  punctuation_present: boolean;
  model_downloading: boolean;
  settings_path: string;
}

/** `set_asr_params` 载荷：可调整的 ASR 引擎/运行参数（snake_case 直传，缺省项不修改）。 */
export interface AsrParamsPatch {
  num_threads?: number;
  chunk_size?: number;
  enable_endpoint?: boolean;
  rule1_min_trailing_silence?: number;
  rule2_min_trailing_silence?: number;
  rule3_min_utterance_length?: number;
  blank_penalty?: number;
  hotwords?: string;
  enable_punctuation?: boolean;
  debug?: boolean;
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

/** `live2d-model-changed` 事件载荷（切换伙伴 / 清屏；清屏时三字段均为 null） */
export interface Live2dModelInfo {
  model_dir: string | null;
  model_file: string | null;
  format: string | null;
}

/** `list_companions` / `set_active_companion` 里的单个伙伴 */
export interface CompanionModelInfo {
  id: string;
  name: string;
  /** 原始导入目录（仅记录来源；运行时不依赖，源删除后伙伴仍有效） */
  source_path: string | null;
  /** 应用托管目录 `~/.zapmomo/companions/{id}` */
  model_dir: string;
  /** 托管目录内的 .model3.json 绝对路径 */
  model_file: string;
  format: string;
  imported_at: string;
  /** 快速有效判定：托管目录与清单文件是否都还在磁盘上 */
  valid: boolean;
  /** 探测到的封面图绝对路径（无封面图为 null，列表用占位图标） */
  cover_image: string | null;
}

/** `list_companions` / `set_active_companion` 返回的伙伴库视图 */
export interface CompanionLibraryView {
  models: CompanionModelInfo[];
  active_model_id: string | null;
}

/** `import_companion` 返回 */
export interface ImportCompanionResult {
  library: CompanionLibraryView;
  model_id: string;
  already_imported: boolean;
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
  /** 扩散解码步数（质量/速度权衡），可经 `set_tts_params` 修改 */
  num_steps: number;
  /** 默认语速，可经 `set_tts_params` 修改 */
  speed: number;
  /** 调试输出，可经 `set_tts_params` 修改 */
  debug: boolean;
}

/** `set_tts_params` 载荷：可调整的 TTS 合成参数（snake_case 直传，缺省项不修改）。 */
export interface TtsParamsPatch {
  num_steps?: number;
  speed?: number;
  num_threads?: number;
  debug?: boolean;
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
  /** 是否为用户自定义音色（true = 来自音色库，false = 模型包内置） */
  custom: boolean;
}

/** `save_tts_voice` 载荷：把源 wav 拷贝进音色库并登记。 */
export type SaveTtsVoiceRequest = {
  name: string;
  sourceWavPath: string;
  referenceText: string;
};

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
  /** RuntimeActual：当前真正加载的模型路径（None = 未加载） */
  loaded_model_path: string | null;
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

// ---- 语音会话（KWS→ASR→LLM→TTS 全链路）----

/** `voice-session-state` 事件的会话阶段 */
export type VoiceSessionPhase =
  | "idle"
  | "armed"
  | "greeting"
  | "waiting_speech"
  | "listening"
  | "thinking"
  | "speaking";

/** `voice-session-state` 事件载荷 */
export interface VoiceSessionStatePayload {
  running: boolean;
  state: VoiceSessionPhase;
}

/** `voice-session-wake` 事件载荷 */
export interface VoiceWake {
  keyword: string;
}

/** `voice-session-transcript` 事件载荷（ASR 实时字幕） */
export interface VoiceTranscript {
  text: string;
  is_final: boolean;
}

/** `voice-session-token` 事件载荷（LLM 流式增量） */
export interface VoiceToken {
  delta: string;
}

/** `voice-session-reply` 事件载荷（切句入队合成） */
export interface VoiceReplySentence {
  sentence: string;
}

/** `voice-session-play` 事件载荷（正在播报的句子） */
export interface VoicePlaySentence {
  sentence: string;
}

/** `voice-session-reply-finished` 事件载荷 */
export interface VoiceReplyFinished {
  reason: string;
}

/** `voice-session-error` 事件载荷 */
export interface VoiceError {
  message: string;
}

/** `voice-session-stopped` 事件载荷 */
export interface VoiceStopped {
  error: string | null;
}
