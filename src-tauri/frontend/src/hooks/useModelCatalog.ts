import { useInfiniteQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";
import { buildCatalogQuery, type CatalogUiState, debounce } from "@/lib/catalog/query";
import { api } from "@/lib/tauri";
import type { CatalogSort, ModelCategory, ParameterRange, UnifiedModelItem } from "@/types/catalog";

const SEARCH_DEBOUNCE_MS = 400;

const defaultUi: CatalogUiState = {
  category: null,
  search: "",
  language: null,
  license: null,
  parameters: null,
  sort: "recommended",
};

export interface ModelCatalogState {
  items: UnifiedModelItem[];
  loading: boolean;
  loadingMore: boolean;
  error: string | null;
  hasMore: boolean;
  query: CatalogUiState;
  /** 是否显示全部模型（含"可能兼容/不兼容"），默认 false = 只显示 Verified + Compatible。 */
  showAll: boolean;
  toggleShowAll: () => void;
  setQuery: (patch: Partial<CatalogUiState>) => void;
  setCategory: (cat: ModelCategory | null) => void;
  setSearch: (s: string) => void;
  setSort: (s: CatalogSort) => void;
  setLanguage: (l: string | null) => void;
  setLicense: (l: string | null) => void;
  setParameters: (p: ParameterRange | null) => void;
  loadMore: () => void;
  retry: () => void;
  refresh: () => void;
}

/**
 * 模型目录分页（@tanstack/react-query useInfiniteQuery）。
 *
 * - queryKey = 查询条件（含 debounced search）+ confirmedOnly：条件变化自动重新查询，
 *   旧请求由 React Query 取消/丢弃（无需手写 stale token）。
 * - 缓存 / 重试 / 取消 / 加载态由 React Query 标准化管理。
 */
export function useModelCatalog(): ModelCatalogState {
  const [ui, setUi] = useState<CatalogUiState>(defaultUi);
  const [showAll, setShowAll] = useState(false);
  // 搜索 debounce：输入即时更新 ui.search，queryKey 用 committedSearch（避免每次按键都请求）
  const [committedSearch, setCommittedSearch] = useState(defaultUi.search);

  useEffect(() => {
    const d = debounce(() => setCommittedSearch(ui.search), SEARCH_DEBOUNCE_MS);
    d.run();
    return () => d.cancel();
  }, [ui.search]);

  const queryUi = useMemo(() => ({ ...ui, search: committedSearch }), [ui, committedSearch]);

  const infinite = useInfiniteQuery({
    queryKey: ["catalog", queryUi],
    queryFn: ({ pageParam }) =>
      api.catalogSearchModels("huggingface", buildCatalogQuery(queryUi, pageParam)),
    initialPageParam: 0,
    getNextPageParam: (last, allPages) => (last.hasMore ? allPages.length : undefined),
    staleTime: 5 * 60_000,
  });

  /**
   * 合并所有页并去重（按 canonicalKey）。
   * 注意：merge_catalog 会把内置精选注入每一页，flatMap 会重复；这里去重只保留一份。
   * 同时统计"最后一页新增的可见项数"（当前筛选下），用于耗尽保护。
   */
  const { items, lastPageNewVisible } = useMemo(() => {
    const seen = new Set<string>();
    const out: UnifiedModelItem[] = [];
    const pages = infinite.data?.pages ?? [];
    const lastIdx = pages.length - 1;
    let newVisible = 0;
    pages.forEach((p, pi) => {
      for (const i of p.items) {
        if (seen.has(i.canonicalKey)) continue;
        seen.add(i.canonicalKey);
        out.push(i);
        if (
          pi === lastIdx &&
          (showAll || i.compatibility === "verified" || i.compatibility === "compatible")
        ) {
          newVisible += 1;
        }
      }
    });
    return { items: out, lastPageNewVisible: newVisible };
  }, [infinite.data, showAll]);

  /**
   * 耗尽保护：已加载多页且最后一页无新增可见项
   * （如 ASR/TTS/KWS 的 HF 结果全是 Unsupported）→ 停止自动加载，
   * 避免哨兵反复触发、同一批数据重复请求。
   */
  const exhausted = useMemo(
    () => (infinite.data?.pages.length ?? 0) > 1 && lastPageNewVisible === 0,
    [infinite.data, lastPageNewVisible],
  );

  const loadMore = useCallback(() => {
    if (infinite.hasNextPage && !infinite.isFetchingNextPage) {
      void infinite.fetchNextPage();
    }
  }, [infinite.hasNextPage, infinite.isFetchingNextPage, infinite.fetchNextPage]);

  const refresh = useCallback(() => {
    void infinite.refetch();
  }, [infinite.refetch]);

  const setQuery = useCallback((patch: Partial<CatalogUiState>) => {
    setUi((prev) => ({ ...prev, ...patch }));
  }, []);

  const setCategory = useCallback(
    (cat: ModelCategory | null) => setUi((p) => ({ ...p, category: cat })),
    [],
  );
  const setSearch = useCallback((s: string) => setUi((p) => ({ ...p, search: s })), []);
  const setSort = useCallback((s: CatalogSort) => setUi((p) => ({ ...p, sort: s })), []);
  const setLanguage = useCallback((l: string | null) => setUi((p) => ({ ...p, language: l })), []);
  const setLicense = useCallback((l: string | null) => setUi((p) => ({ ...p, license: l })), []);
  const setParameters = useCallback(
    (pa: ParameterRange | null) => setUi((p) => ({ ...p, parameters: pa })),
    [],
  );
  const toggleShowAll = useCallback(() => setShowAll((v) => !v), []);

  return {
    items,
    loading: infinite.isLoading,
    loadingMore: infinite.isFetchingNextPage,
    error: infinite.error ? String(infinite.error) : null,
    hasMore: infinite.hasNextPage && !exhausted,
    query: ui,
    showAll,
    toggleShowAll,
    setQuery,
    setCategory,
    setSearch,
    setSort,
    setLanguage,
    setLicense,
    setParameters,
    loadMore,
    retry: refresh,
    refresh,
  };
}
