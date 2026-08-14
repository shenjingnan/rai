import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import { ConfigCard } from "@/components/ConfigCard";
import { Header } from "@/components/Header";
import { ListenCard } from "@/components/ListenCard";
import { ResultsCard } from "@/components/ResultsCard";
import { useAppInfo } from "@/hooks/useAppInfo";
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

  // macOS 为非透明原生窗口（tauri.macos.conf.json），圆角由系统绘制；
  // 其它平台仍是透明窗口，需要 CSS 圆角裁出圆角。
  const isMac = navigator.userAgent.includes("Macintosh");

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
      <Header info={info} isListening={listening.isListening} />
      <main className="mx-auto grid w-full max-w-3xl flex-1 gap-4 overflow-y-auto p-5">
        <ListenCard
          devices={devices.devices}
          devicesError={devices.error}
          onRefreshDevices={devices.refresh}
          isListening={listening.isListening}
          error={listening.error}
          onStart={listening.start}
          onStop={listening.stop}
        />
        <ConfigCard config={config.config} error={config.error} onRefresh={config.refresh} />
        <ResultsCard results={results} />
      </main>
    </div>
  );
}
