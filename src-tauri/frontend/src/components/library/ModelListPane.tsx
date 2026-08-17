import { Loader2, Search, WifiOff } from "lucide-react";
import { useEffect, useRef } from "react";
import { useInView } from "react-intersection-observer";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import type { UnifiedModelItem } from "@/types/catalog";
import { ModelListCard } from "./ModelListCard";

interface ModelListPaneProps {
  items: UnifiedModelItem[];
  loading: boolean;
  loadingMore: boolean;
  error: string | null;
  hasMore: boolean;
  selectedId: string | null;
  onSelect: (item: UnifiedModelItem) => void;
  onRetry: () => void;
  onLoadMore: () => void;
}

/** 左列表（55%）：无限滚动 + 骨架/错误/空态。HF 失败时仍展示本地/已验证（错误态只影响在线段）。 */
export function ModelListPane(props: ModelListPaneProps) {
  const onLoadMoreRef = useRef(props.onLoadMore);
  onLoadMoreRef.current = props.onLoadMore;
  // 自动填满已加载页数上限（防请求风暴）
  const autoFillRef = useRef(0);

  // react-intersection-observer：sentinel 进入视口（含 200px 余量）→ inView 变 true。
  // inView 只在可见性状态变化时更新，加载完成后不变 → 不会连环触发。
  const { ref: sentinelRef, inView } = useInView({ rootMargin: "200px" });

  // 触底加载：sentinel 可见且还有下一页 → loadMore。
  // 加载完成后 inView 不变 → 不重复触发；用户离开再滚回（inView false→true）才再次加载。
  useEffect(() => {
    if (inView && props.hasMore && !props.loading && !props.loadingMore) {
      onLoadMoreRef.current();
    }
  }, [inView, props.hasMore, props.loading, props.loadingMore]);

  // 续载：加载完成后若 sentinel 仍可见（列表不足一屏 / 用户停在底部）→ 自动补页，最多 3 页。
  // 避免「只加载一页就停」；之后需用户滚动触发上方 effect。
  useEffect(() => {
    if (!inView || props.loading || props.loadingMore || !props.hasMore) return;
    if (autoFillRef.current >= 3) return;
    autoFillRef.current += 1;
    onLoadMoreRef.current();
  }, [inView, props.loading, props.loadingMore, props.hasMore]);

  if (props.loading && props.items.length === 0) {
    return (
      <div className="flex h-full flex-col gap-2 rounded-[16px] border border-panel-border bg-panel-background p-3">
        {[0, 1, 2, 3, 4].map((i) => (
          <Skeleton key={i} className="h-20 w-full rounded-[14px]" />
        ))}
      </div>
    );
  }

  if (props.error && props.items.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 rounded-[16px] border border-panel-border bg-panel-background px-6 py-14 text-center">
        <WifiOff className="h-8 w-8 text-text-muted" />
        <p className="text-sm font-medium text-text-primary">无法连接 Hugging Face</p>
        <p className="text-xs text-text-secondary">你仍然可以管理已经下载的本地模型。</p>
        <Button variant="outline" size="sm" className="mt-1 shadow-none" onClick={props.onRetry}>
          重新加载
        </Button>
      </div>
    );
  }

  if (props.items.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 rounded-[16px] border border-panel-border bg-panel-background px-6 py-14 text-center">
        <Search className="h-8 w-8 text-text-muted" />
        <p className="text-sm font-medium text-text-primary">没有找到符合条件的模型</p>
        <p className="text-xs text-text-secondary">尝试调整搜索内容或筛选条件</p>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {props.error && (
        <p className="mb-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-600">
          在线目录加载部分失败：{props.error}（本地模型仍可用）
        </p>
      )}
      {/* 外边框容器固定，内部卡片滚动；分隔线 + 底部加载指示 */}
      <div className="min-h-0 flex-1 divide-y divide-divider overflow-y-auto rounded-[16px] border border-panel-border bg-panel-background">
        {props.items.map((item) => (
          <ModelListCard
            key={item.canonicalKey}
            item={item}
            selected={props.selectedId === item.canonicalKey}
            onSelect={props.onSelect}
          />
        ))}
        <div ref={sentinelRef} className="flex justify-center py-1">
          {props.loadingMore && <Loader2 className="h-4 w-4 animate-spin text-text-muted" />}
          {!props.hasMore && props.items.length > 0 && (
            <span className="text-xs text-text-muted">已到底部</span>
          )}
          {props.hasMore && !props.loadingMore && (
            <span className={cn("text-xs text-text-muted")}>滚动加载更多…</span>
          )}
        </div>
      </div>
    </div>
  );
}
