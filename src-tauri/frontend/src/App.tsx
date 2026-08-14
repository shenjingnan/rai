import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { AsrCard } from "@/components/AsrCard";
import { ConfigCard } from "@/components/ConfigCard";
import { DeviceCard } from "@/components/DeviceCard";
import { Header } from "@/components/Header";
import { ListenCard } from "@/components/ListenCard";
import { Live2dCard } from "@/components/Live2dCard";
import { ResultsCard } from "@/components/ResultsCard";
import { useAppInfo } from "@/hooks/useAppInfo";
import { useAsrListening } from "@/hooks/useAsrListening";
import { useDevices } from "@/hooks/useDevices";
import { useKwsConfig } from "@/hooks/useKwsConfig";
import { useListening } from "@/hooks/useListening";
import { useResults } from "@/hooks/useResults";
import { cn } from "@/lib/utils";

export default function App() {
  const info = useAppInfo();
  const devices = useDevices();
  const config = useKwsConfig();
  const listening = useListening();
  const results = useResults();
  const asrListening = useAsrListening();
  const [device, setDevice] = useState("");

  // macOS 为非透明原生窗口（tauri.macos.conf.json），圆角由系统绘制；
  // 其它平台仍是透明窗口，需要 CSS 圆角裁出圆角。
  const isMac = navigator.userAgent.includes("Macintosh");

  // 任一识别/监听进行中时，禁止切换输入设备。
  const anyListening = listening.isListening || asrListening.isListening;

  useEffect(() => {
    if (info) document.title = `${info.product_name} · KWS 控制面板`;
  }, [info]);

  // macOS 窗口默认 visible:false，首帧渲染完成后显示，避免白屏闪烁。
  useEffect(() => {
    if (isMac) getCurrentWindow().show();
  }, [isMac]);

  return (
    <div
      className={cn(
        "flex h-screen flex-col overflow-hidden bg-background text-foreground",
        !isMac && "rounded-xl",
      )}
    >
      <Header info={info} isListening={anyListening} />
      <main className="mx-auto grid w-full max-w-3xl flex-1 gap-4 overflow-y-auto p-5">
        <DeviceCard
          devices={devices.devices}
          error={devices.error}
          value={device}
          onChange={setDevice}
          onRefresh={devices.refresh}
          disabled={anyListening}
        />
        <ListenCard
          isListening={listening.isListening}
          error={listening.error}
          onStart={(keywords) => listening.start(device || null, keywords)}
          onStop={listening.stop}
        />
        <ConfigCard config={config.config} error={config.error} onRefresh={config.refresh} />
        <ResultsCard results={results} />
        <AsrCard
          isListening={asrListening.isListening}
          error={asrListening.error}
          onStart={() => asrListening.start(device || null)}
          onStop={asrListening.stop}
        />
        <Live2dCard />
      </main>
    </div>
  );
}
