import { X } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/** 退出动画时长，需与遮罩/卡片 duration 一致。 */
const EXIT_MS = 200;

interface LibraryDialogProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  /** 底部操作区（确认框的取消/确认按钮等） */
  footer?: ReactNode;
  maxWidth?: string;
}

/** 模型库通用对话框外壳：复用项目现有 `role="dialog"` + 遮罩 + 进出场动画模式。 */
export function LibraryDialog({
  open,
  onClose,
  title,
  children,
  footer,
  maxWidth = "max-w-lg",
}: LibraryDialogProps) {
  const [mounted, setMounted] = useState(open);
  const [closing, setClosing] = useState(false);

  useEffect(() => {
    if (open) {
      setMounted(true);
      setClosing(false);
      return;
    }
    // 父级已通过 onClose 把 open 置为 false（如底部确认/取消按钮直接调用）：
    // 播放退出动画并卸载，避免残留「空内容」的挂载弹窗；不再重复调用 onClose。
    if (mounted) {
      setClosing(true);
      const timer = window.setTimeout(() => {
        setMounted(false);
        setClosing(false);
      }, EXIT_MS);
      return () => window.clearTimeout(timer);
    }
  }, [open, mounted]);

  const finishClose = useCallback(() => {
    setMounted(false);
    setClosing(false);
    onClose();
  }, [onClose]);

  const close = useCallback(() => {
    if (closing) return;
    setClosing(true);
    window.setTimeout(finishClose, EXIT_MS);
  }, [closing, finishClose]);

  useEffect(() => {
    if (!mounted || closing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [mounted, closing, close]);

  if (!mounted) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      <button
        type="button"
        tabIndex={-1}
        aria-label="关闭对话框"
        className={cn(
          "absolute inset-0 cursor-default bg-black/20",
          closing ? "animate-out fade-out-0 duration-200" : "animate-in fade-in-0 duration-200",
        )}
        onClick={close}
      />
      <div
        className={cn(
          "relative flex max-h-[85vh] w-full flex-col rounded-xl border border-panel-border bg-panel-background",
          maxWidth,
          closing
            ? "animate-out fade-out-0 zoom-out-95 duration-200 ease-in"
            : "animate-in fade-in-0 zoom-in-95 duration-200 ease-out",
        )}
      >
        <div className="flex items-center justify-between gap-4 border-b border-divider px-5 py-4">
          <h3 className="text-sm font-semibold text-text-primary">{title}</h3>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 shrink-0"
            onClick={close}
            aria-label="关闭"
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
        <div className="flex-1 space-y-3 overflow-y-auto px-5 py-4">{children}</div>
        {footer && <div className="border-t border-divider px-5 py-3">{footer}</div>}
      </div>
    </div>
  );
}
