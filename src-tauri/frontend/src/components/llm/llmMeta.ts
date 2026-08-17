import type { LlmConfigInfo } from "@/types/tauri";

/** 状态语义色：绿=ready、蓝=loading/生成中、灰=未配置/未加载、红=错误。 */
export type LlmStatusTone = "good" | "loading" | "idle" | "error";

export const STATUS_COLOR: Record<LlmStatusTone, string> = {
  good: "text-emerald-600",
  loading: "text-blue-600",
  idle: "text-text-muted",
  error: "text-red-600",
};

/** 从 model_path 派生展示名：取 basename 并去掉 `.gguf`。空路径返回 null，不硬编码任何模型名。 */
export function modelNameFromPath(modelPath: string | undefined | null): string | null {
  if (!modelPath) return null;
  const base = modelPath.split(/[\\/]/).pop() ?? modelPath;
  return base.endsWith(".gguf") ? base.slice(0, -".gguf".length) : base;
}

/** 当前模型展示名：仅当模型文件真实存在（models_present）时从路径派生，否则 null（未配置模型）。 */
export function currentModelName(cfg: LlmConfigInfo | null): string | null {
  if (!cfg?.models_present) return null;
  return modelNameFromPath(cfg.model_path);
}

/** 是否为 OpenAI 兼容的远程 provider（openai / llamacpp-server）。 */
export function isHttpProvider(provider: string | undefined | null): boolean {
  return provider === "openai" || provider === "llamacpp-server";
}

/**
 * 第 4 列「状态」完整状态机（判断顺序：错误 > 加载中 > 生成中 > 已加载 > 未加载 > 未配置模型）。
 * `configError`（get_llm_config 失败）不进入此状态机，由调用方单独展示。
 */
export function llmStatus(
  cfg: LlmConfigInfo | null,
  st: { ready: boolean; loading: boolean; generating: boolean; error: string | null },
): { tone: LlmStatusTone; label: string } {
  if (st.error) return { tone: "error", label: "错误" };
  if (st.loading) return { tone: "loading", label: "加载中" };
  if (st.generating) return { tone: "loading", label: "生成中" };
  if (st.ready) return { tone: "good", label: "已加载" };
  if (cfg?.models_present) return { tone: "idle", label: "未加载" };
  return { tone: "idle", label: "未配置模型" };
}
