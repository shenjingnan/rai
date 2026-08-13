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

export default function App() {
  const info = useAppInfo();
  const devices = useDevices();
  const config = useKwsConfig();
  const listening = useListening();
  const results = useResults();

  useEffect(() => {
    if (info) document.title = `${info.product_name} · KWS 控制面板`;
  }, [info]);

  return (
    <div className="flex h-screen flex-col overflow-hidden rounded-xl bg-background text-foreground">
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
