/** 状态语义色：绿=识别中、蓝=启动中、灰=未识别、红=错误。 */
export type AsrStatusTone = "good" | "loading" | "idle" | "error";

export const ASR_STATUS_COLOR: Record<AsrStatusTone, string> = {
  good: "text-emerald-600",
  loading: "text-blue-600",
  idle: "text-text-muted",
  error: "text-red-600",
};

/** 从 model_dir 派生展示名：取 basename。空路径返回 null，不硬编码任何模型名。 */
export function modelNameFromDir(dir: string | null | undefined): string | null {
  if (!dir) return null;
  return dir.split(/[\\/]/).pop() ?? dir;
}

/** 默认（legacy 一键下载所装的）双语 ASR 模型目录名前缀（不同版本安装布局的日期后缀可能不同）。 */
const DEFAULT_ASR_DIR_PREFIX = "sherpa-onnx-streaming-zipformer-bilingual-zh-en";

/**
 * 当前模型目录是否为默认双语模型：决定模型缺失时展示 legacy「下载模型」
 * （固定下载双语 + 标点）还是「选择模型」弹窗（可下载/切换其他 ASR 模型）。
 */
export function isDefaultAsrModelDir(dir: string | null | undefined): boolean {
  const name = modelNameFromDir(dir);
  return !!name && name.startsWith(DEFAULT_ASR_DIR_PREFIX);
}

/**
 * ASR 运行状态机（判断顺序：错误 > 启动中 > 识别中 > 未识别）。
 * 顶部 Switch 文字用此 label 反映真实识别状态。
 * `configError`（get_asr_config 失败）不进入此状态机，由调用方单独展示。
 */
export function asrStatus(st: { isListening: boolean; pending: boolean; error: string | null }): {
  tone: AsrStatusTone;
  label: string;
} {
  if (st.error) return { tone: "error", label: "错误" };
  if (st.pending) return { tone: "loading", label: "启动中" };
  if (st.isListening) return { tone: "good", label: "识别中" };
  return { tone: "idle", label: "未识别" };
}
