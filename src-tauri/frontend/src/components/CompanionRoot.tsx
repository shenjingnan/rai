import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import { Live2dStage } from "@/components/live2d/Live2dStage";
import { VoiceStatusDot } from "@/components/voice/VoiceStatusDot";
import { useLive2dConfig } from "@/hooks/useLive2dConfig";
import { useVoiceSession } from "@/hooks/useVoiceSession";
import { api, onCompanionScaleChanged, onLive2dModelChanged, toAssetUrl } from "@/lib/tauri";

/** 角色窗口基准高度上限（100% 时高度 = min(480, 屏幕可用高度 × 0.6)）。 */
const BASE_HEIGHT = 480;
/** 窗口尺寸下限与初始值。 */
const MIN_WIDTH = 120;
const MIN_HEIGHT = 120;
const INITIAL_WIDTH = 360;
/** 模型宽高比缺省值（3:4），模型加载后更新为真实宽高比。 */
const DEFAULT_ASPECT_RATIO = 3 / 4;
/** 缩放比例范围（25% ~ 200%）。 */
const SCALE_MIN = 0.25;
const SCALE_MAX = 2.0;
/** cmd/ctrl + 滚轮单格缩放步长。 */
const WHEEL_SCALE_STEP = 1.1;

/**
 * 常驻角色窗口：静态展示 Live2D 模型（仅呼吸/眨眼等自动动画，不跟随鼠标）。
 *
 * - 启动时读 `get_live2d_config` 恢复持久化的模型与缩放比例；
 * - 订阅 `live2d-model-changed` / `companion-scale-changed`，设置窗口切换模型或缩放时即时同步；
 * - 窗口尺寸由「基准高度 × scale」派生（宽度按模型宽高比），缩放入口为设置面板、cmd/ctrl+滚轮
 *   与原生右键菜单（后端弹原生菜单，不受小窗口裁剪）；
 * - 按住左键拖动移动窗口；右键弹出原生上下文菜单。
 */
export function CompanionRoot() {
  const containerRef = useRef<HTMLDivElement>(null);
  const { config } = useLive2dConfig();
  // 桌宠窗口无 RuntimeContext：hook 自包含，与设置窗口订阅同一批后端 voice 事件。
  const voice = useVoiceSession();
  const [modelUrl, setModelUrl] = useState<string | null>(null);
  const [aspectRatio, setAspectRatio] = useState(DEFAULT_ASPECT_RATIO);
  const [scale, setScale] = useState(1.0);
  const [size, setSize] = useState({ width: INITIAL_WIDTH, height: BASE_HEIGHT });

  // 用 ref 保存最新值，供异步回调（滚轮/事件/模型加载）读取，避免闭包过期。
  const aspectRatioRef = useRef(aspectRatio);
  aspectRatioRef.current = aspectRatio;
  const scaleRef = useRef(scale);
  scaleRef.current = scale;

  /** 由「基准高度 × scale × 宽高比」计算窗口尺寸（逻辑像素），并 clamp 到屏幕可用区域。 */
  const computeSize = useCallback((ratio: number, s: number) => {
    const availW = window.screen.availWidth;
    const availH = window.screen.availHeight;
    const baseH = Math.min(BASE_HEIGHT, availH * 0.6);
    let height = Math.round(baseH * s);
    let width = Math.round(height * ratio);
    height = Math.max(MIN_HEIGHT, Math.min(height, Math.floor(availH * 0.9)));
    width = Math.max(MIN_WIDTH, Math.min(width, Math.floor(availW * 0.9)));
    return { width, height };
  }, []);

  /** 统一设置窗口尺寸并同步本地 state。 */
  const resizeTo = useCallback(
    async (ratio: number, s: number) => {
      const { width, height } = computeSize(ratio, s);
      await getCurrentWindow().setSize(new LogicalSize(width, height));
      setSize({ width, height });
    },
    [computeSize],
  );

  /** 用户缩放：更新 scale、resize 并持久化比例。 */
  const applyScale = useCallback(
    async (s: number) => {
      const clamped = Math.max(SCALE_MIN, Math.min(SCALE_MAX, s));
      setScale(clamped);
      await resizeTo(aspectRatioRef.current, clamped);
      await api.setCompanionScale({ scale: clamped });
    },
    [resizeTo],
  );
  const applyScaleRef = useRef(applyScale);
  applyScaleRef.current = applyScale;

  // 启动时恢复持久化的模型（顺带重放行 asset 协议 scope）。
  useEffect(() => {
    if (config?.models_present && config.model_file) {
      setModelUrl(toAssetUrl(config.model_file));
    }
  }, [config]);

  // 恢复持久化的缩放比例，并据此 resize 一次（确保前端 state 与后端建窗尺寸一致）。
  useEffect(() => {
    if (!config) return;
    const s = config.window_scale ?? 1.0;
    setScale(s);
    void resizeTo(aspectRatioRef.current, s);
  }, [config, resizeTo]);

  useEffect(() => {
    const unlisten = onLive2dModelChanged((info) => {
      // 空 model_file = 清屏（active 伙伴被移除等场景）。
      setModelUrl(info.model_file ? toAssetUrl(info.model_file) : null);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // 设置面板改比例时同步（只 resize，不再写回后端避免循环）。
  useEffect(() => {
    const unlisten = onCompanionScaleChanged((s) => {
      setScale(s);
      void resizeTo(aspectRatioRef.current, s);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [resizeTo]);

  // 模型加载后更新真实宽高比，并用当前 scale 重算尺寸（不持久化 scale）。
  const handleModelMetrics = useCallback(
    async (metrics: { aspectRatio: number }) => {
      const ratio =
        Number.isFinite(metrics.aspectRatio) && metrics.aspectRatio > 0
          ? metrics.aspectRatio
          : DEFAULT_ASPECT_RATIO;
      setAspectRatio(ratio);
      await resizeTo(ratio, scaleRef.current);
    },
    [resizeTo],
  );

  // 监听窗口移动：拖动停止（debounce）后把逻辑像素坐标写回 settings，供下次启动恢复。
  useEffect(() => {
    const win = getCurrentWindow();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unlisten = win.onMoved(({ payload }) => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        void (async () => {
          const factor = await win.scaleFactor();
          const x = Math.round(payload.x / factor);
          const y = Math.round(payload.y / factor);
          await api.saveCompanionPosition({ x, y });
        })();
      }, 300);
    });
    return () => {
      clearTimeout(timer);
      void unlisten.then((fn) => fn());
    };
  }, []);

  // cmd/ctrl + 滚轮：连续缩放（节流约 100ms，阻止默认滚动）。
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const onWheel = (e: WheelEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      e.preventDefault();
      if (timer) return;
      const step = e.deltaY < 0 ? WHEEL_SCALE_STEP : 1 / WHEEL_SCALE_STEP;
      const next = scaleRef.current * step;
      if (next < SCALE_MIN || next > SCALE_MAX) return;
      timer = setTimeout(() => {
        timer = undefined;
      }, 100);
      void applyScaleRef.current(next);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      el.removeEventListener("wheel", onWheel);
      if (timer) clearTimeout(timer);
    };
  }, []);

  return (
    <div
      ref={containerRef}
      role="application"
      className="relative h-screen w-screen select-none overflow-hidden bg-transparent"
      onMouseDown={(e) => {
        if (e.button !== 0) return;
        void getCurrentWindow().startDragging();
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        void api.showCompanionMenu({ x: e.clientX, y: e.clientY });
      }}
    >
      <Live2dStage
        modelUrl={modelUrl}
        width={size.width}
        height={size.height}
        onModelMetrics={handleModelMetrics}
      />
      <span className="absolute right-2 top-2">
        <VoiceStatusDot phase={voice.phase} running={voice.running} />
      </span>
    </div>
  );
}
