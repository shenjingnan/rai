import {
  ArrowRight,
  AudioWaveform,
  Brain,
  MessageSquare,
  MessageSquareText,
  Mic,
  Volume2,
} from "lucide-react";
import { useState } from "react";
import { CapabilityRow, type StatusTone } from "@/components/models/CapabilityRow";
import { ConfirmDialog } from "@/components/models/ConfirmDialog";
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
  const [kwsConfirmOpen, setKwsConfirmOpen] = useState(false);

  const asrStatus = listenerStatus(asrListening.error, asrOn, asrConfig?.config?.models_present);
  const kwsStatus = listenerStatus(kwsListening.error, kwsOn, kwsConfig?.config?.models_present);
  const llmStatusNow = llmStatus(llm.error, llm.loading, llm.ready, llm.config?.models_present);

  /** KWS 开关：ASR 未开启时先弹确认框，同意则同时开启 ASR 与 KWS。 */
  const handleKwsToggle = () => {
    if (kwsOn) {
      void kwsListening.stop();
      return;
    }
    if (asrOn) {
      void kwsListening.start(device || null, null);
      return;
    }
    setKwsConfirmOpen(true);
  };

  const enableKwsWithAsr = () => {
    setKwsConfirmOpen(false);
    void asrListening.start(device || null);
    void kwsListening.start(device || null, null);
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
              toggleHint={!asrOn && !kwsOn ? "点击开启将同时启用语音输入" : undefined}
              tooltip="关键词唤醒：说出唤醒词后开始对话，依赖语音输入（ASR）。"
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
              statusText={tts.config?.models_present ? "● 已就绪" : "○ 未配置模型"}
              statusTone={tts.config?.models_present ? "on" : "off"}
              tooltip="语音合成：模型就绪后可在「语音合成」中手动将文字转为语音。"
            />
          </div>
        </div>
      </div>

      <ConfirmDialog
        open={kwsConfirmOpen}
        title="需要先开启语音输入"
        description="唤醒词（KWS）依赖语音识别（ASR）。是否同时开启语音识别与唤醒词？"
        confirmText="同时开启"
        onConfirm={enableKwsWithAsr}
        onCancel={() => setKwsConfirmOpen(false)}
      />
    </section>
  );
}
