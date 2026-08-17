/** 模型库（Model Library）类型定义，与 Rust `model_library` 的 camelCase 序列化一一对应。 */

export type ModelType = "kws" | "asr" | "llm" | "tts";
export type ModelSource = "registry" | "local";
export type StorageOwnership = "managed" | "external";
export type InstallState = "not_installed" | "downloading" | "installed" | "invalid";
export type RuntimeStatus = "inactive" | "active" | "switching" | "pending_restart" | "load_failed";
export type RuntimeAction =
  | "none"
  | "reloaded"
  | "restart_required"
  | "reload_failed_rolled_back"
  | "reload_failed_rollback_failed";

export interface LibraryModel {
  id: string;
  name: string;
  displayName: string;
  modelType: ModelType;
  runtime: string;
  format: string;
  description: string;
  languages: string[];
  tags: string[];
  parameterCount: string | null;
  quantization: string | null;
  version: string;
  sizeBytes: number | null;
  homepage: string | null;
  /** 是否有内置下载源（false = LLM 需导入 GGUF） */
  downloadable: boolean;
  source: ModelSource;
  ownership: StorageOwnership;
  installState: InstallState;
  /** 是否为该能力当前选择的模型（RuntimeSelection） */
  current: boolean;
  /** 运行状态（仅 current 模型有意义） */
  runtimeStatus: RuntimeStatus;
  localPath: string | null;
  installedAt: string | null;
  /** 稳定安装身份（set_current_model / delete_model 按此定位具体 Artifact） */
  installId: string | null;
  /** HF repo_id（若可映射） */
  repoId: string | null;
  /** 兼容性级别（verified/compatible/possible/unsupported） */
  compatibility: string | null;
}

export interface SystemResources {
  totalMemory: number;
  availableMemory: number;
  diskTotal: number;
  diskAvailable: number;
  cpuUsage: number;
}

export interface SetCurrentResult {
  modelType: ModelType;
  modelId: string;
  path: string;
  runtimeAction: RuntimeAction;
  effectiveImmediately: boolean;
  message: string;
}

export type LibraryProgressStage =
  | "preparing"
  | "downloading"
  | "verifying"
  | "extracting"
  | "done"
  | "cancelled"
  | "failed";

export interface ModelLibraryProgress {
  modelId: string;
  stage: LibraryProgressStage;
  asset: string;
  overallPercent: number;
  bytesDownloaded: number;
  totalBytes: number;
  message: string;
}
