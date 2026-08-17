import type { KwsConfigInfo } from "@/types/tauri";

/** 状态语义色：绿=监听中、灰=未监听/未下载、红=错误。 */
export type KwsStatusTone = "good" | "idle" | "error";

export const KWS_STATUS_COLOR: Record<KwsStatusTone, string> = {
  good: "text-emerald-600",
  idle: "text-text-muted",
  error: "text-red-600",
};

/** 从 model_dir 派生展示名：取 basename。空路径返回 null，不硬编码任何模型名。 */
export function modelNameFromDir(dir: string | null | undefined): string | null {
  if (!dir) return null;
  return dir.split(/[\\/]/).pop() ?? dir;
}

/**
 * KWS 状态语义色（判断顺序：错误 > 监听中 > 未监听 > 未下载模型）。
 * 标题栏开关旁的文字显示「启用/禁用 KWS」，圆点颜色用此 tone 反映真实监听状态。
 * `configError`（get_kws_config 失败）不进入此状态机，由调用方单独展示。
 */
export function kwsStatus(
  cfg: KwsConfigInfo | null,
  st: { isListening: boolean; error: string | null },
): { tone: KwsStatusTone } {
  if (st.error) return { tone: "error" };
  if (st.isListening) return { tone: "good" };
  if (cfg?.models_present) return { tone: "idle" };
  return { tone: "idle" };
}
