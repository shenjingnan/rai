import { Info } from "lucide-react";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 当前模式提示：根据各能力真实状态派生，只描述实际可用的输入/能力，
 * 不承诺「语音回复」等当前业务尚未实现的链路。
 */
export function ModeBanner() {
  const {
    asr: { listening: asrListening },
    kws: { listening: kwsListening },
    llm,
    tts,
  } = useRuntime();

  const asrOn = asrListening.isListening;
  const kwsOn = kwsListening.isListening;
  const llmOn = llm.ready;
  const ttsReady = tts.config?.models_present ?? false;

  let modeTitle: string;
  let modeDesc: string;
  if (asrOn && kwsOn) {
    modeTitle = "唤醒式语音输入";
    modeDesc = "通过唤醒词开始，语音会实时转写为文字；也可键盘输入。";
  } else if (asrOn) {
    modeTitle = "语音 + 文字输入";
    modeDesc = "可直接说话，语音会实时转写为文字；也可键盘输入。";
  } else {
    modeTitle = "文字输入";
    modeDesc = "通过键盘与 AI 对话（文字输入始终可用）。";
  }

  const hints: string[] = [];
  if (!asrOn) {
    hints.push("当前未启用语音输入，你仍然可以通过键盘与 AI 对话。");
  } else if (!kwsOn) {
    hints.push("当前可使用语音输入；启用唤醒词后可通过唤醒词开始对话。");
  }
  if (!llmOn && llm.config?.models_present) {
    hints.push("AI 大脑已配置但未加载。");
  }
  if (!llmOn && !llm.config?.models_present) {
    hints.push("AI 大脑未配置模型，暂无法生成回复。");
  }
  if (ttsReady) {
    hints.push("TTS 模型已就绪：可在「语音合成」中把文字转为语音。");
  } else {
    hints.push("TTS 模型未配置：语音合成暂不可用。");
  }

  return (
    <div className="flex items-start gap-3 rounded-xl border border-blue-200 bg-blue-50/60 px-4 py-3 text-sm dark:border-blue-900 dark:bg-blue-950/40">
      <Info className="mt-0.5 h-4 w-4 shrink-0 text-blue-600 dark:text-blue-400" />
      <div className="min-w-0">
        <p className="text-text-primary">
          <span className="font-medium">当前模式：{modeTitle}</span>
          <span className="text-text-secondary"> · {modeDesc}</span>
        </p>
        {hints.length > 0 && (
          <ul className="mt-1 space-y-0.5 text-text-secondary">
            {hints.map((hint) => (
              <li key={hint}>· {hint}</li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
