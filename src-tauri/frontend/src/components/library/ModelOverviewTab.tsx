import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { RemoteModelDetail, UnifiedModelItem } from "@/types/catalog";

interface ModelOverviewTabProps {
  item: UnifiedModelItem;
  detail: RemoteModelDetail | null;
}

/** 概览：description + 元数据 + README（懒加载；纯文本 <pre>，无 Markdown renderer，防注入）。 */
export function ModelOverviewTab({ item, detail }: ModelOverviewTabProps) {
  const [readme, setReadme] = useState<string | null>(null);
  const [readmeLoading, setReadmeLoading] = useState(false);

  const description = item.remote?.description ?? item.builtin?.description ?? detail?.description;

  useEffect(() => {
    if (!item.remote || readme || readmeLoading) return;
    setReadmeLoading(true);
    void api
      .catalogGetModelReadme("huggingface", item.modelId, item.remote.sha ?? null)
      .then((r) => setReadme(r))
      .catch(() => setReadme(null))
      .finally(() => setReadmeLoading(false));
  }, [item, readme, readmeLoading]);

  const metaRows: [string, string][] = [
    ["仓库", item.modelId],
    ["标签", [...(item.remote?.tags ?? item.builtin?.tags ?? [])].slice(0, 8).join(" · ")],
    ["语言", (item.remote?.languages ?? item.builtin?.languages ?? []).join(" / ")],
    ["许可证", item.remote?.license ?? ""],
  ].filter(([, v]) => !!v) as [string, string][];

  return (
    <div className="space-y-3">
      {description && <p className="text-sm leading-relaxed text-text-primary">{description}</p>}
      {metaRows.length > 0 && (
        <dl className="space-y-1">
          {metaRows.map(([k, v]) => (
            <div key={k} className="flex items-baseline gap-2 text-xs">
              <dt className="w-16 shrink-0 text-text-muted">{k}</dt>
              <dd className="min-w-0 break-all text-text-primary">{v}</dd>
            </div>
          ))}
        </dl>
      )}
      {item.remote && (
        <div>
          <p className="mb-1 text-xs font-medium text-text-secondary">README</p>
          {readmeLoading ? (
            <p className="text-xs text-text-muted">加载中…</p>
          ) : readme ? (
            /* 完整展示 README（无内部滚动条；超长时由抽屉整体滚动） */
            <pre className="whitespace-pre-wrap break-words rounded-lg border border-panel-border bg-app-background p-3 text-xs leading-relaxed text-text-secondary">
              {readme}
            </pre>
          ) : (
            <p className="text-xs text-text-muted">无 README 或未公开</p>
          )}
        </div>
      )}
    </div>
  );
}
