import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "@/lib/tauri";
import type {
  ModelCompatibility,
  RemoteModelDetail,
  RemoteModelFile,
  UnifiedModelItem,
} from "@/types/catalog";

export interface ModelDetailState {
  selected: UnifiedModelItem | null;
  detail: RemoteModelDetail | null;
  compatibility: ModelCompatibility | null;
  files: RemoteModelFile[] | null;
  loading: boolean;
  loadingFiles: boolean;
  error: string | null;
  filesError: string | null;
  select: (item: UnifiedModelItem) => void;
  close: () => void;
  ensureCompatibility: () => void;
  ensureFiles: () => void;
  refresh: () => void;
}

/**
 * 模型详情：点选 → detail（元数据）；需要 Variant/兼容性时 `ensureCompatibility()`（内部走
 * catalog_get_compatibility，files 由 Rust 共享缓存，Variant/Files/Compatibility 不重复请求）。
 * README 懒加载由 Overview Tab 自行触发。
 */
export function useModelDetail(): ModelDetailState {
  const [selected, setSelected] = useState<UnifiedModelItem | null>(null);
  const [detail, setDetail] = useState<RemoteModelDetail | null>(null);
  const [compatibility, setCompatibility] = useState<ModelCompatibility | null>(null);
  const [files, setFiles] = useState<RemoteModelFile[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filesError, setFilesError] = useState<string | null>(null);

  const modelIdRef = useRef<string | null>(null);
  const revRef = useRef<string | null>(null);

  const isCurrentModel = useCallback(
    (item: UnifiedModelItem | null) => item?.modelId === modelIdRef.current,
    [],
  );

  const select = useCallback(
    (item: UnifiedModelItem) => {
      setSelected(item);
      modelIdRef.current = item.modelId;
      revRef.current = item.remote?.sha ?? null;
      setDetail(null);
      setCompatibility(null);
      setFiles(null);
      setError(null);
      setFilesError(null);
      setLoading(true);
      void api
        .catalogGetModelDetail("huggingface", item.modelId, item.remote?.sha ?? null)
        .then((d) => {
          if (!isCurrentModel(item)) return;
          setDetail(d);
          // 若 HF repo 无 sha，用 detail 的 sha 作为 revision
          revRef.current = d.sha ?? null;
        })
        .catch((e) => {
          if (isCurrentModel(item)) setError(String(e));
        })
        .finally(() => {
          if (isCurrentModel(item)) setLoading(false);
        });
    },
    [isCurrentModel],
  );

  const ensureCompatibility = useCallback(() => {
    const modelId = modelIdRef.current;
    if (!modelId || compatibility) return;
    setLoadingFiles(true);
    setFilesError(null);
    void api
      .catalogGetCompatibility("huggingface", modelId, revRef.current)
      .then((c) => setCompatibility(c))
      .catch((e) => setFilesError(String(e)))
      .finally(() => setLoadingFiles(false));
  }, [compatibility]);

  const ensureFiles = useCallback(() => {
    const modelId = modelIdRef.current;
    if (!modelId || files) return;
    setLoadingFiles(true);
    setFilesError(null);
    void api
      .catalogGetModelFiles("huggingface", modelId, revRef.current)
      .then((f) => setFiles(f))
      .catch((e) => setFilesError(String(e)))
      .finally(() => setLoadingFiles(false));
  }, [files]);

  const refresh = useCallback(() => {
    const item = selected;
    if (item) select(item);
  }, [selected, select]);

  /** 关闭详情抽屉：清空选择与缓存状态。 */
  const close = useCallback(() => {
    modelIdRef.current = null;
    revRef.current = null;
    setSelected(null);
    setDetail(null);
    setCompatibility(null);
    setFiles(null);
    setError(null);
    setFilesError(null);
    setLoading(false);
    setLoadingFiles(false);
  }, []);

  // 清理：selected 变化后，若旧模型仍残留，直接覆盖（由 select 重置）
  useEffect(() => {
    return () => {
      modelIdRef.current = null;
    };
  }, []);

  return {
    selected,
    detail,
    compatibility,
    files,
    loading,
    loadingFiles,
    error,
    filesError,
    select,
    close,
    ensureCompatibility,
    ensureFiles,
    refresh,
  };
}
