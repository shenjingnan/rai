import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogOut, Settings } from "lucide-react";
import { useEffect, useMemo } from "react";
import { useLive2dConfig } from "@/hooks/useLive2dConfig";
import { api, toAssetUrl } from "@/lib/tauri";
import { Live2dStage } from "@/pet/Live2dStage";

const PET_WIDTH = 420;
const PET_HEIGHT = 560;

/** 桌宠窗口：透明悬浮 + 拖拽 + 呼出设置面板。 */
export function PetApp() {
  const { config } = useLive2dConfig();

  const modelUrl = useMemo(() => {
    if (config?.models_present && config.model_file) {
      return toAssetUrl(config.model_file);
    }
    return null;
  }, [config]);

  // pet 窗口配置 visible:false，首帧渲染后显示，避免白屏闪烁。
  useEffect(() => {
    void getCurrentWindow().show();
  }, []);

  const openSettings = async () => {
    const main = await WebviewWindow.getByLabel("main");
    await main?.show();
    await main?.setFocus();
  };

  return (
    <div data-tauri-drag-region className="relative h-screen w-screen overflow-hidden">
      {modelUrl ? (
        <Live2dStage
          modelUrl={modelUrl}
          width={PET_WIDTH}
          height={PET_HEIGHT}
          className="h-full w-full"
        />
      ) : (
        <div className="flex h-full w-full items-center justify-center px-6">
          <p className="text-center text-sm text-muted-foreground">
            尚未加载角色模型
            <br />按 Cmd+, 打开设置面板选择模型
          </p>
        </div>
      )}

      <div className="absolute bottom-3 right-3 flex gap-2">
        <button
          type="button"
          onClick={openSettings}
          title="设置（Cmd+,）"
          className="rounded-full bg-background/80 p-2 shadow-sm transition-colors hover:bg-background"
        >
          <Settings className="h-4 w-4 text-foreground" />
        </button>
        <button
          type="button"
          onClick={() => void api.quitApp()}
          title="退出"
          className="rounded-full bg-background/80 p-2 shadow-sm transition-colors hover:bg-background"
        >
          <LogOut className="h-4 w-4 text-foreground" />
        </button>
      </div>
    </div>
  );
}
