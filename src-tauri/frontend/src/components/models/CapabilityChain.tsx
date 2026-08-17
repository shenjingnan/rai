import {
  ArrowRight,
  AudioWaveform,
  Brain,
  MessageSquare,
  MessageSquareText,
  Mic,
  Volume2,
} from "lucide-react";
import { CapabilityRow, type StatusTone } from "@/components/models/CapabilityRow";
import { useRuntime } from "@/providers/RuntimeContext";

/** 计算 KWS/ASR 这类「监听型」能力的展示状态。 */
function listenerStatus(
  error: string | null,
  active: boolean,
  modelsPresent: boolean | undefined,
): { text: string; tone: StatusTone } {
  if (error) return { text: "● 错误", tone: "error" };
  if (active) return { text: "● 开启", tone: "on" };
  if (modelsPresent) return { text: "○ 关闭", tone: "off" };
  return { text: "○ 未配置模型", tone: "off" };
}

/** 计算 LLM 展示状态。 */
function llmStatus(
  error: string | null,
  loading: boolean,
  ready: boolean,
  modelsPresent: boolean | undefined,
): { text: string; tone: StatusTone } {
  if (error) return { text: "● 错误", tone: "error" };
  if (loading) return { text: "● 加载中", tone: "loading" };
  if (ready) return { text: "● 开启", tone: "on" };
  if (modelsPresent) return { text: "○ 关闭", tone: "off" };
  return { text: "○ 未配置模型", tone: "off" };
}

/** AI 能力链路：输入 → AI 大脑 → 输出 三段式，全部绑定真实 runtime 状态。 */
export function CapabilityChain() {
  const {
    kws: { config: kwsConfig, listening: kwsListening },
    asr: { config: asrConfig, listening: asrListening },
    llm,
    tts,
    device,
  } = useRuntime();

  const asrOn = asrListening.isListening;
  const kwsOn = kwsListening.isListening;

  const asrStatus = listenerStatus(asrListening.error, asrOn, asrConfig?.config?.models_present);
  const kwsStatus = listenerStatus(kwsListening.error, kwsOn, kwsConfig?.config?.models_present);
  const llmStatusNow = llmStatus(llm.error, llm.loading, llm.ready, llm.config?.models_present);

  /** KWS 开关：直接开始/停止 KWS（与 ASR 相互独立，后端不强关联）。 */
  const handleKwsToggle = () => {
    if (kwsOn) {
      void kwsListening.stop();
    } else {
      void kwsListening.start(device || null, null);
    }
  };

  return (
    <section className="rounded-[16px] border border-panel-border bg-panel-background p-3">
      <h2 className="text-base font-semibold text-text-primary">AI 能力链路</h2>

      <div className="mt-2 flex items-stretch gap-2.5">
        {/* 输入 */}
        <div className="min-w-0 flex-1 rounded-[14px] border border-panel-border px-3.5 py-1">
          <h3 className="pt-2 text-xs font-medium text-text-muted">输入</h3>
          <div className="divide-y divide-[#eef1f6]">
            <CapabilityRow
              accent="violet"
              icon={MessageSquare}
              name="文字输入"
              code="Text"
              description="通过键盘输入，始终可用"
              statusText="● 始终可用"
              statusTone="always"
              tooltip="文字输入始终可用，不依赖任何模型。"
            />
            <CapabilityRow
              accent="blue"
              icon={Mic}
              name="语音输入"
              code="ASR"
              description="将你的语音转换为文字"
              statusText={asrStatus.text}
              statusTone={asrStatus.tone}
              toggled={asrOn}
              onToggle={() => (asrOn ? asrListening.stop() : asrListening.start(device || null))}
              tooltip="语音识别：将麦克风语音实时转写为文字。"
            />
            <CapabilityRow
              accent="violet"
              icon={AudioWaveform}
              name="唤醒词"
              code="KWS"
              description="检测唤醒词，辅助开始语音对话"
              statusText={kwsStatus.text}
              statusTone={kwsStatus.tone}
              toggled={kwsOn}
              onToggle={handleKwsToggle}
              tooltip="关键词唤醒：检测到唤醒词后触发事件，可与语音输入配合实现语音对话。"
            />
          </div>
        </div>

        <ArrowRight className="h-5 w-5 shrink-0 self-center text-text-muted" />

        {/* AI 大脑 */}
        <div className="w-[230px] shrink-0 rounded-[14px] border border-panel-border px-3.5 py-1">
          <h3 className="pt-2 text-xs font-medium text-text-muted">AI 大脑</h3>
          <div className="divide-y divide-[#eef1f6]">
            <CapabilityRow
              accent="green"
              icon={Brain}
              name="AI 大脑"
              code="LLM"
              description="理解你的话并生成回复"
              statusText={llmStatusNow.text}
              statusTone={llmStatusNow.tone}
              toggled={llm.ready}
              onToggle={() => (llm.ready ? llm.unload() : llm.load())}
              toggleDisabled={llm.loading || !llm.config?.models_present}
              tooltip="本地大模型：用 llama.cpp 在本地生成回复。"
            />
          </div>
        </div>

        <ArrowRight className="h-5 w-5 shrink-0 self-center text-text-muted" />

        {/* 输出 */}
        <div className="min-w-0 flex-1 rounded-[14px] border border-panel-border px-3.5 py-1">
          <h3 className="pt-2 text-xs font-medium text-text-muted">输出</h3>
          <div className="divide-y divide-[#eef1f6]">
            <CapabilityRow
              accent="violet"
              icon={MessageSquareText}
              name="文字显示"
              code="Text"
              description="在界面中显示文字回复"
              statusText="● 始终可用"
              statusTone="always"
              tooltip="文字回复始终在对话窗口中显示。"
            />
            <CapabilityRow
              accent="orange"
              icon={Volume2}
              name="语音合成"
              code="TTS"
              description="将文字转换为自然语音"
              statusText={
                tts.config?.enabled === false
                  ? "○ 已关闭"
                  : tts.config?.models_present
                    ? "● 已就绪"
                    : "○ 未配置模型"
              }
              statusTone={
                tts.config?.enabled === false ? "off" : tts.config?.models_present ? "on" : "off"
              }
              toggled={tts.config?.enabled ?? true}
              onToggle={() => tts.setEnabled(!(tts.config?.enabled ?? true))}
              tooltip="语音合成：模型就绪后可在「语音合成」中手动将文字转为语音；关闭后合成将被禁用。"
            />
          </div>
        </div>
      </div>
    </section>
  );
}
