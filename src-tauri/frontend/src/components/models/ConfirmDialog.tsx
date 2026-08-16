import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description: string;
  confirmText: string;
  cancelText?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/** 退出动画时长，需与卡片/遮罩的 duration 一致。 */
const EXIT_MS = 200;

/** 轻量确认对话框：模态遮罩 + 居中卡片，进出场带动画，支持 Esc / 遮罩取消。 */
export function ConfirmDialog({
  open,
  title,
  description,
  confirmText,
  cancelText = "取消",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const [mounted, setMounted] = useState(open);
  const [closing, setClosing] = useState(false);

  // open 变 true 时挂载并播放入场动画
  useEffect(() => {
    if (open) {
      setMounted(true);
      setClosing(false);
    }
  }, [open]);

  const finishClose = useCallback((action: () => void) => {
    setMounted(false);
    setClosing(false);
    action();
  }, []);

  const close = useCallback(
    (action: () => void) => {
      if (closing) return;
      setClosing(true);
      window.setTimeout(() => finishClose(action), EXIT_MS);
    },
    [closing, finishClose],
  );

  const handleCancel = useCallback(() => close(onCancel), [close, onCancel]);
  const handleConfirm = useCallback(() => close(onConfirm), [close, onConfirm]);

  // Esc 取消
  useEffect(() => {
    if (!mounted || closing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [mounted, closing, handleCancel]);

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
        onClick={handleCancel}
      />
      <div
        className={cn(
          "relative w-full max-w-sm rounded-xl border border-panel-border bg-panel-background p-5",
          closing
            ? "animate-out fade-out-0 zoom-out-95 duration-200 ease-in"
            : "animate-in fade-in-0 zoom-in-95 duration-200 ease-out",
        )}
      >
        <h3 className="text-sm font-semibold text-text-primary">{title}</h3>
        <p className="mt-1.5 text-sm text-text-secondary">{description}</p>
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={handleCancel}>
            {cancelText}
          </Button>
          <Button size="sm" onClick={handleConfirm}>
            {confirmText}
          </Button>
        </div>
      </div>
    </div>
  );
}
