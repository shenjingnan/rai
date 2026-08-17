/** 模型目录（Catalog）类型定义，与 Rust `model_library::catalog` camelCase 序列化对应。 */

export type ModelCategory = "llm" | "asr" | "tts" | "kws";

export type CatalogSort = "recommended" | "downloads" | "likes" | "last_modified" | "trending";

export type ParameterRange = "under_1b" | "b1_to_3" | "b3_to_7" | "b7_to_14" | "over_14";

export interface CatalogQuery {
  category?: ModelCategory | null;
  search?: string | null;
  language?: string | null;
  license?: string | null;
  parameters?: ParameterRange | null;
  sort: CatalogSort;
  page: number;
  pageSize: number;
  includeUnsupported: boolean;
}

export type CompatibilityLevel = "verified" | "compatible" | "possible" | "unsupported";

/** HF 列表页一条模型（Summary）。 */
export interface RemoteModelSummary {
  repoId: string;
  author: string;
  displayName: string;
  description: string | null;
  pipelineTag: string | null;
  libraryName: string | null;
  tags: string[];
  downloads: number;
  likes: number;
  trendingScore: number | null;
  lastModified: string | null;
  createdAt: string | null;
  license: string | null;
  languages: string[];
  parameterCount: string | null;
  gated: string | null;
  private: boolean | null;
  sha: string | null;
}

/** 模型详情（repo 元数据；文件走 catalog_get_model_files）。 */
export interface RemoteModelDetail {
  repoId: string;
  description: string | null;
  pipelineTag: string | null;
  libraryName: string | null;
  tags: string[];
  license: string | null;
  languages: string[];
  downloads: number;
  likes: number;
  lastModified: string | null;
  createdAt: string | null;
  sha: string | null;
  gated: string | null;
  private: boolean | null;
  cardData: unknown | null;
  siblings: string[];
}

export interface FileLfs {
  sha256: string;
  size: number;
}

/** 文件树条目（懒加载）。 */
export interface RemoteModelFile {
  path: string;
  size: number | null;
  type: "file" | "directory";
  lfs: FileLfs | null;
  sha256: string | null;
}

/** 一个可下载安装单元（LLM=1..N gguf；sherpa=文件组）。 */
export interface ModelArtifact {
  id: string;
  name: string;
  runtime: string;
  format: string;
  variant: string | null;
  files: RemoteModelFile[];
  totalSize: number | null;
  installable: boolean;
}

/** 兼容性判定结果。 */
export interface ModelCompatibility {
  level: CompatibilityLevel;
  reason: string;
  modelType: ModelCategory | null;
  architecture: string | null;
  artifacts: ModelArtifact[];
  recommendedVariant: string | null;
}

/** 模型级本地状态聚合（UI summary）。 */
export interface LocalModelSummary {
  installedArtifactCount: number;
  hasCurrentArtifact: boolean;
  activeDownloadCount: number;
}

/** 列表级本地安装视图。 */
export interface LocalInstallView {
  installId: string;
  artifactId: string;
  variant: string | null;
  state: string;
  isCurrent: boolean;
  localPath: string | null;
}

/** 内置精选展示信息（来自 registry，非 HF 数据）。 */
export interface BuiltinModelSummary {
  displayName: string;
  description: string;
  modelType: ModelCategory;
  runtime: string;
  format: string;
  languages: string[];
  tags: string[];
  parameterCount: string | null;
  sizeBytes: number | null;
}

/** 统一模型条目（HF + Verified + Local 三源合并，canonical key 去重）。 */
export interface UnifiedModelItem {
  canonicalKey: string;
  modelId: string;
  provider: string;
  remote: RemoteModelSummary | null;
  builtin: BuiltinModelSummary | null;
  modelType: ModelCategory | null;
  compatibility: CompatibilityLevel;
  compatibilityNotes: string | null;
  recommendedVariant: string | null;
  installs: LocalInstallView[];
  localSummary: LocalModelSummary;
  confirmed: boolean;
}

export interface CatalogPage<T> {
  items: T[];
  hasMore: boolean;
}

/** 下载任务请求（camelCase，与 Rust download.rs 对应）。 */
export interface DownloadArtifactRequest {
  modelId: string;
  artifactId: string;
  variant?: string | null;
  artifactSource: string;
  repoId?: string | null;
  revision?: string | null;
  files: RemoteModelFile[];
  modelType?: ModelCategory | null;
}

/** 下载任务视图（独立 taskId）。 */
export interface DownloadTaskView {
  taskId: string;
  modelId: string;
  artifactId: string;
  variant: string | null;
  artifactSource: string;
  state: string;
  currentFile: string | null;
  fileIndex: number;
  fileTotal: number;
  bytesDownloaded: number;
  totalBytes: number;
  progress: number;
  queuePosition: number;
  queueLength: number;
}
