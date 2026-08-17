import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { api } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * GGUF 模型文件选择动作与错误态。
 * 供「选择模型」入口复用 `open()` → `set_llm_model_path` 逻辑。
 */
export function useLlmModelPicker() {
  const { llm } = useRuntime();
  const [pickError, setPickError] = useState<string | null>(null);

  const pick = async () => {
    const path = await open({
      multiple: false,
      title: "选择 GGUF 模型",
      filters: [{ name: "GGUF", extensions: ["gguf"] }],
    });
    if (typeof path !== "string") return; // 用户取消对话框
    setPickError(null);
    try {
      await api.setLlmModelPath({ path });
      // 无缝切换：若旧模型已加载且路径确实变化，后端 load 会重建引擎（卸载旧模型、加载新模型）
      if (llm.ready && path !== llm.config?.model_path) {
        await llm.load();
      }
      await llm.refreshConfig();
    } catch (e) {
      setPickError(String(e));
    }
  };

  return { pick, pickError };
}
