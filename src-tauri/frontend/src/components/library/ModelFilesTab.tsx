import { File as FileIcon, Folder } from "lucide-react";
import { formatBytes } from "@/components/library/libraryMeta";
import type { RemoteModelFile } from "@/types/catalog";

/** 文件列表（懒加载；GGUF 高亮；不做虚拟列表）。 */
export function ModelFilesTab({ files }: { files: RemoteModelFile[] }) {
  if (files.length === 0) {
    return <p className="py-6 text-center text-xs text-text-muted">没有文件信息</p>;
  }
  return (
    <ul className="divide-y divide-divider rounded-xl border border-panel-border bg-panel-background">
      {files.map((f) => (
        <li key={f.path} className="flex items-center gap-2.5 px-3 py-2">
          {f.type === "directory" ? (
            <Folder className="h-3.5 w-3.5 shrink-0 text-amber-500" />
          ) : (
            <FileIcon className="h-3.5 w-3.5 shrink-0 text-text-muted" />
          )}
          <span
            className={
              "min-w-0 flex-1 truncate font-mono text-xs " +
              (f.path.toLowerCase().endsWith(".gguf") ? "text-blue-600" : "text-text-primary")
            }
          >
            {f.path}
          </span>
          <span className="shrink-0 text-[11px] text-text-muted">
            {f.size != null ? formatBytes(f.size) : ""}
          </span>
        </li>
      ))}
    </ul>
  );
}
