import { useCallback, useEffect, useState } from "react";
import { api, onCatalogDownloadProgress } from "@/lib/tauri";
import type { DownloadArtifactRequest, DownloadTaskView } from "@/types/catalog";

export interface ModelDownloadsState {
  tasks: Map<string, DownloadTaskView>;
  enqueue: (request: DownloadArtifactRequest) => Promise<DownloadTaskView>;
  cancel: (taskId: string) => Promise<void>;
  refresh: () => Promise<void>;
}

/**
 * 下载队列：订阅 `download-progress` 事件，维护 `Map<taskId, DownloadTaskView>`。
 * 注意：绝不使用 repoId 作为 key（同 repo 多 variant 可同时排队）。
 */
export function useModelDownloads(): ModelDownloadsState {
  const [tasks, setTasks] = useState<Map<string, DownloadTaskView>>(new Map());

  const applyView = useCallback((view: DownloadTaskView) => {
    setTasks((prev) => {
      const next = new Map(prev);
      if (view.state === "cancelled" || view.state === "failed" || view.state === "done") {
        // 终态保留一小段时间供 UI 展示，之后由 refresh 清理
        next.set(view.taskId, view);
      } else {
        next.set(view.taskId, view);
      }
      return next;
    });
  }, []);

  useEffect(() => {
    const unlisten = onCatalogDownloadProgress(applyView);
    void api.downloadSnapshot().then((snap) => {
      setTasks(new Map(snap.map((t) => [t.taskId, t])));
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [applyView]);

  const enqueue = useCallback(
    async (request: DownloadArtifactRequest) => {
      const view = await api.downloadEnqueue(request);
      applyView(view);
      return view;
    },
    [applyView],
  );

  const cancel = useCallback(
    async (taskId: string) => {
      await api.downloadCancel(taskId);
      applyView({ taskId, state: "cancelled" } as DownloadTaskView);
    },
    [applyView],
  );

  const refresh = useCallback(async () => {
    const snap = await api.downloadSnapshot();
    setTasks(new Map(snap.map((t) => [t.taskId, t])));
  }, []);

  return { tasks, enqueue, cancel, refresh };
}
