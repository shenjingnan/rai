import { getCurrentWindow, LogicalPosition, LogicalSize } from "@tauri-apps/api/window";
import type { Live2DModel } from "pixi-live2d-display/cubism4";
import { useCallback, useEffect, useRef, useState } from "react";
import { EventBubble } from "@/components/companion/EventBubble";
import { Live2dStage } from "@/components/live2d/Live2dStage";
import { VoiceStatusDot } from "@/components/voice/VoiceStatusDot";
import { useLive2dConfig } from "@/hooks/useLive2dConfig";
import { useVoiceSession } from "@/hooks/useVoiceSession";
import { pickMotionGroup } from "@/lib/dshMotion";
import {
  api,
  onCompanionLayerChanged,
  onCompanionLockedChanged,
  onCompanionOpacityChanged,
  onCompanionScaleChanged,
  onDshSpeak,
  onLive2dModelChanged,
  toAssetUrl,
} from "@/lib/tauri";
import { centeredResizeTarget } from "@/lib/windowResize";
import type { CompanionWindowLayer } from "@/types/tauri";

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
 * - 启动时读 `get_live2d_config` 恢复持久化的模型、缩放比例与透明度；
 * - 订阅 `live2d-model-changed` / `companion-scale-changed` / `companion-opacity-changed`，
 *   设置窗口切换模型、缩放或调透明度时即时同步（透明度由包裹模型的 wrapper div 的
 *   `style.opacity` 应用，语音状态点不受影响）；
 * - 窗口尺寸由「基准高度 × scale」派生（宽度按模型宽高比），缩放入口为设置面板、cmd/ctrl+滚轮
 *   与原生右键菜单（后端弹原生菜单，不受小窗口裁剪）；
 * - 按住左键拖动移动窗口（位置锁定时禁止）；右键弹出原生上下文菜单。
 */
export function CompanionRoot() {
  const containerRef = useRef<HTMLDivElement>(null);
  const { config } = useLive2dConfig();
  // 桌宠窗口无 RuntimeContext：hook 自包含，与设置窗口订阅同一批后端 voice 事件。
  const voice = useVoiceSession();
  const [modelUrl, setModelUrl] = useState<string | null>(null);
  const [aspectRatio, setAspectRatio] = useState(DEFAULT_ASPECT_RATIO);
  const [scale, setScale] = useState(1.0);
  const [opacity, setOpacity] = useState(1.0);
  // 显示层级：置底（back）为纯背景装饰（点穿、不可拖拽/右键/滚轮），置顶（front）为现状浮层。
  const [layer, setLayer] = useState<CompanionWindowLayer>("front");
  // 位置锁定：禁止拖动窗口（滚轮缩放与右键菜单保留，右键菜单是解锁入口）。
  const [locked, setLocked] = useState(false);
  const [size, setSize] = useState({ width: INITIAL_WIDTH, height: BASE_HEIGHT });

  // Live2D 模型句柄：dsh 事件触发动作用（模型缺对应组时静默跳过）。
  const modelRef = useRef<Live2DModel | null>(null);

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

  /** 统一设置窗口尺寸并同步本地 state；以窗口中心为锚点，角色缩放时保持原位。 */
  const resizeTo = useCallback(
    async (ratio: number, s: number) => {
      const win = getCurrentWindow();
      const { width, height } = computeSize(ratio, s);
      // setSize 默认固定左上角（向右下生长），会使居中的角色表现为从左上角缩放。
      // 读取当前物理位置/尺寸并换算，把左上角移到「保持窗口中心不变」的位置。
      const factor = await win.scaleFactor();
      const pos = await win.outerPosition();
      const cur = await win.outerSize();
      const target = centeredResizeTarget(
        { x: pos.x, y: pos.y, width: cur.width, height: cur.height },
        Math.round(width * factor),
        Math.round(height * factor),
      );
      // 同时发送尺寸+位置（不逐个 await）：避免「先变尺寸、再瞬移归位」的中间态，减少缩放抖动。
      const sizeOp = win.setSize(new LogicalSize(width, height));
      const posOp = win
        .setPosition(
          new LogicalPosition(Math.round(target.x / factor), Math.round(target.y / factor)),
        )
        .catch((e) => {
          // 中心锚定失败（如权限缺失）时降级为默认锚定；尺寸已生效，仍要更新布局避免模型被裁。
          console.warn("中心锚定 setPosition 失败，已降级为默认锚定:", e);
        });
      await Promise.allSettled([sizeOp, posOp]);
      setSize({ width, height });
    },
    [computeSize],
  );

  /** 用户缩放：更新 scale、resize 并持久化比例。 */
  const applyScale = useCallback(
    async (s: number) => {
      const clamped = Math.max(SCALE_MIN, Math.min(SCALE_MAX, s));
      scaleRef.current = clamped; // 立即同步，连续滚轮基于最新值计算下一步
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

  // 恢复持久化的缩放比例与透明度，并据此 resize 一次（确保前端 state 与后端建窗尺寸一致）。
  useEffect(() => {
    if (!config) return;
    const s = config.window_scale ?? 1.0;
    setScale(s);
    setOpacity(config.window_opacity ?? 1.0);
    if (config.window_layer) setLayer(config.window_layer);
    // 旧后端 / 测试桩可能不返回该字段，兜底为未锁定。
    setLocked(config.locked ?? false);
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
      // 本窗口滚轮缩放后也会收到这条回显（applyScale → setCompanionScale），值相同则跳过，
      // 避免每次滚轮触发两轮 resize/重布局造成抖动。
      if (s === scaleRef.current) return;
      setScale(s);
      void resizeTo(aspectRatioRef.current, s);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [resizeTo]);

  // 设置面板/菜单改透明度时同步（纯视觉：只更新渲染层 opacity，不涉及窗口尺寸）。
  useEffect(() => {
    const unlisten = onCompanionOpacityChanged((v) => {
      setOpacity(v);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // 设置面板切显示层级时同步（置底：隐藏状态点、关闭交互；置顶：恢复）。
  useEffect(() => {
    const unlisten = onCompanionLayerChanged((l) => {
      setLayer(l);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // 设置面板/菜单锁定位置时同步（只拦截拖动，不影响缩放与右键）。
  useEffect(() => {
    const unlisten = onCompanionLockedChanged((v) => {
      setLocked(v);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

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

  // dsh 任务事件：气泡由 EventBubble 渲染，这里联动触发模型动作。
  useEffect(() => {
    const unlisten = onDshSpeak(({ event }) => {
      const model = modelRef.current;
      if (!model) return;
      // motionManager 类型上非空，但运行时缺组/初始化异常时可能缺失，防御跳过。
      if (!model.internalModel.motionManager) return;
      const groups = Object.keys(
        (model.internalModel.motionManager.definitions ?? {}) as Record<string, unknown>,
      );
      const group = pickMotionGroup(groups, event.type);
      if (!group) return;
      // FORCE 优先级（3）：打断 idle/在播动作，同 previewManager 的 startMotion 语义
      void model.internalModel.motionManager.startMotion(group, 0, 3);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

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

  // cmd/ctrl + 滚轮：连续缩放（节流约 60ms，阻止默认滚动）。
  // 置底（back）为点穿背景，不挂滚轮监听（原生层本已吞掉鼠标事件，这里是防御）。
  useEffect(() => {
    if (layer === "back") return;
    const el = containerRef.current;
    if (!el) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const onWheel = (e: WheelEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      e.preventDefault();
      if (timer) return;
      // 步长随滚轮实际位移变化：鼠标一格 deltaY≈100 → 1.1×；小幅位移 = 微调，缩放更平滑。
      const next = scaleRef.current * WHEEL_SCALE_STEP ** (e.deltaY / 100);
      if (next < SCALE_MIN || next > SCALE_MAX) return;
      timer = setTimeout(() => {
        timer = undefined;
      }, 60);
      void applyScaleRef.current(next);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      el.removeEventListener("wheel", onWheel);
      if (timer) clearTimeout(timer);
    };
  }, [layer]);

  return (
    <div
      ref={containerRef}
      role="application"
      className="relative h-screen w-screen select-none overflow-hidden bg-transparent"
      onMouseDown={(e) => {
        if (e.button !== 0 || layer === "back" || locked) return;
        void getCurrentWindow().startDragging();
      }}
      onContextMenu={(e) => {
        if (layer === "back") return;
        e.preventDefault();
        void api.showCompanionMenu({ x: e.clientX, y: e.clientY });
      }}
    >
      {/* 透明度只作用于模型本身，语音状态点保持不透明 */}
      <div style={{ opacity }}>
        <Live2dStage
          modelUrl={modelUrl}
          width={size.width}
          height={size.height}
          onModelMetrics={handleModelMetrics}
          onModelLoaded={(m) => {
            modelRef.current = m;
          }}
        />
      </div>
      {/* dsh 任务事件气泡（pointer-events-none，不挡拖动/右键） */}
      <EventBubble />
      {/* 置底为纯背景装饰，不显示语音状态点 */}
      {layer === "front" && (
        <span className="absolute right-2 top-2">
          <VoiceStatusDot phase={voice.phase} running={voice.running} />
        </span>
      )}
    </div>
  );
}
