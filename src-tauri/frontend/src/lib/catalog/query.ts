/** 目录查询纯函数：UI 状态 → CatalogQuery；分类/兼容性展示映射（Provider-Neutral）。 */
import type {
  CatalogQuery,
  CatalogSort,
  CompatibilityLevel,
  ModelCategory,
  ParameterRange,
} from "@/types/catalog";

/** 分类 Tab → 展示名。 */
export const CATEGORY_LABELS: Record<ModelCategory, string> = {
  llm: "LLM",
  asr: "ASR",
  tts: "TTS",
  kws: "KWS",
};

export const SORT_OPTIONS: { value: CatalogSort; label: string }[] = [
  { value: "recommended", label: "推荐" },
  { value: "downloads", label: "下载量" },
  { value: "likes", label: "点赞" },
  { value: "last_modified", label: "最近更新" },
  { value: "trending", label: "Trending" },
];

export const PARAMETER_OPTIONS: { value: ParameterRange; label: string }[] = [
  { value: "under_1b", label: "< 1B" },
  { value: "b1_to_3", label: "1B - 3B" },
  { value: "b3_to_7", label: "3B - 7B" },
  { value: "b7_to_14", label: "7B - 14B" },
  { value: "over_14", label: "14B+" },
];

export interface CatalogUiState {
  category: ModelCategory | null;
  search: string;
  language: string | null;
  license: string | null;
  parameters: ParameterRange | null;
  sort: CatalogSort;
}

/** UI 状态 → CatalogQuery（任何 filter 改变 → page 重置为 0）。 */
export function buildCatalogQuery(state: CatalogUiState, page: number): CatalogQuery {
  return {
    category: state.category,
    search: state.search.trim() ? state.search.trim() : null,
    language: state.language,
    license: state.license,
    parameters: state.parameters,
    sort: state.sort,
    page,
    pageSize: 20,
    includeUnsupported: false,
  };
}

/** 分类 → HF pipeline tag（与 Rust `category_pipeline_filter` 一致；KWS 用 tag 兜底）。 */
export function categoryToPipelineTag(cat: ModelCategory): string {
  switch (cat) {
    case "llm":
      return "text-generation";
    case "asr":
      return "automatic-speech-recognition";
    case "tts":
      return "text-to-speech";
    case "kws":
      return "wake-word";
  }
}

/** 兼容性 → 展示文案。 */
export function compatibilityLabel(level: CompatibilityLevel): string {
  switch (level) {
    case "verified":
      return "已验证";
    case "compatible":
      return "已确认兼容";
    case "possible":
      return "待确认兼容";
    case "unsupported":
      return "不兼容";
  }
}

/** 是否允许一键安装（Verified / Compatible）。 */
export function isInstallable(level: CompatibilityLevel): boolean {
  return level === "verified" || level === "compatible";
}

/** 兼容性排序权重（越大越靠前）。 */
export function compatibilityRank(level: CompatibilityLevel): number {
  switch (level) {
    case "verified":
      return 3;
    case "compatible":
      return 2;
    case "possible":
      return 1;
    case "unsupported":
      return 0;
  }
}

/** 400ms debounce。 */
export function debounce<A extends unknown[]>(
  fn: (...args: A) => void,
  ms: number,
): { run: (...args: A) => void; cancel: () => void } {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return {
    run(...args: A) {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        fn(...args);
      }, ms);
    },
    cancel() {
      if (timer) clearTimeout(timer);
      timer = null;
    },
  };
}
