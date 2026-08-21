import { Ticker } from "pixi.js";
import type { RefObject } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Live2dStageHandle } from "@/components/live2d/Live2dStage";
import {
  api,
  onDeviceChanged,
  onPerformanceStarted,
  onPerformanceStopped,
  toAssetUrl,
} from "@/lib/tauri";
import type { DeviceEventPayload, PerformancePropsInfo, PerformanceScene } from "@/types/tauri";
import { Damper, type Point2 } from "./damper";
import { getSupportedKey } from "./keyNormalize";
import {
  handParams,
  mapCursorToParams,
  mouseButtonParams,
  type ParamMapping,
} from "./paramMapping";

/** 正在按下的键（键盘表演时展示的爪子贴图）。 */
export interface PressedKey {
  key: string;
  url: string;
  hand: "left" | "right";
}

interface SupportEntry {
  url: string;
  hand: "left" | "right";
}

/**
 * 表演引擎：订阅 Rust 侧模拟键鼠事件（`device-changed`），驱动 Live2D 参数直写
 * 与道具贴图层。
 *
 * - 键盘事件走键名归一化 + 同手互斥，低频进 React state（贴图增删）；
 * - 鼠标事件只写 ref（60-120Hz 不触发渲染），由 PIXI Ticker 阻尼插值；
 * - 每帧参数表在 `stage.onParamFrame`（afterMotionUpdate 时序）里用 `writer`
 *   计算并写入，`writer.range`/`writer.set` 构成任意模型的防御性降级；
 * - 停止/清屏时对触碰过的参数逐个 `reset` 归位，随后由模型自身 motion 接管。
 */
export function usePerformance(
  props: PerformancePropsInfo | null,
  stageRef: RefObject<Live2dStageHandle | null>,
): { scene: PerformanceScene | null; pressedKeys: PressedKey[] } {
  const [scene, setScene] = useState<PerformanceScene | null>(null);
  const [pressedKeys, setPressedKeys] = useState<PressedKey[]>([]);

  const sceneRef = useRef(scene);
  sceneRef.current = scene;
  // 支持键 map（键名 → 贴图 URL + 手）与键名集合，随 props 变化重建。
  const supportRef = useRef<Record<string, SupportEntry>>({});
  const supportKeysRef = useRef(new Set<string>());
  // 高频状态只进 ref，不触发 React 渲染。
  const pressedRef = useRef<PressedKey[]>([]);
  const mouseRef = useRef({ leftDown: false, rightDown: false });
  const playAreaRef = useRef<{ x: number; y: number; width: number; height: number } | null>(null);
  const damperRef = useRef(new Damper());
  const posRef = useRef<Point2 | null>(null);
  // 本引擎触碰过的参数 id（停止/清屏时写回默认值）。
  const touchedRef = useRef(new Set<string>());

  const applyPressed = useCallback((next: PressedKey[]) => {
    pressedRef.current = next;
    setPressedKeys(next);
  }, []);

  // props 变化（切换伙伴/清屏）→ 重建支持键 map，并清空表演残留。
  useEffect(() => {
    const map: Record<string, SupportEntry> = {};
    const keys = new Set<string>();
    for (const k of props?.keys ?? []) {
      map[k.key] = { url: toAssetUrl(k.path), hand: k.hand };
      keys.add(k.key);
    }
    supportRef.current = map;
    supportKeysRef.current = keys;
    // 新伙伴的模型参数不同：旧 touched 集合失效，清掉避免误复位新模型参数。
    touchedRef.current.clear();
  }, [props]);

  const resetMouseState = useCallback(() => {
    mouseRef.current = { leftDown: false, rightDown: false };
    posRef.current = null;
    playAreaRef.current = null;
    damperRef.current.reset();
  }, []);

  // device-changed 分发：按当前场景的通道分流（keyboard 需 typing/both，mouse 需 mouse/both），
  // straggler 事件（通道门禁）被忽略。
  const handleDevice = useCallback(
    (payload: DeviceEventPayload) => {
      const current = sceneRef.current;
      switch (payload.kind) {
        case "KeyboardPress":
        case "KeyboardRelease": {
          if (current !== "typing" && current !== "both") return;
          const supported = getSupportedKey(payload.value, supportKeysRef.current);
          if (!supported) return;
          const entry = supportRef.current[supported];
          if (!entry) return;
          if (payload.kind === "KeyboardPress") {
            // 同手互斥：同手已按的键先移除（BongoCat handlePress 语义）。
            applyPressed([
              ...pressedRef.current.filter((p) => p.hand !== entry.hand),
              { key: supported, url: entry.url, hand: entry.hand },
            ]);
          } else {
            applyPressed(pressedRef.current.filter((p) => p.key !== supported));
          }
          return;
        }
        case "MousePress":
        case "MouseRelease": {
          if (current !== "mouse" && current !== "both") return;
          const down = payload.kind === "MousePress";
          if (payload.value === "Left") mouseRef.current.leftDown = down;
          else if (payload.value === "Right") mouseRef.current.rightDown = down;
          return;
        }
        case "MouseMove": {
          if (current !== "mouse" && current !== "both") return;
          damperRef.current.setTarget(payload.value);
          return;
        }
      }
    },
    [applyPressed],
  );

  // 订阅表演控制与设备事件；挂载时用 is_performing 重同步（dev HMR/重载）。
  useEffect(() => {
    const unlisteners: ReturnType<typeof onDeviceChanged>[] = [];
    unlisteners.push(
      onPerformanceStarted((p) => {
        applyPressed([]);
        resetMouseState();
        // 含鼠标通道（mouse 或 both）时初始化光标起点到播放区域中心。
        if ("play_area" in p) {
          playAreaRef.current = p.play_area;
          const center: Point2 = {
            x: p.play_area.x + p.play_area.width / 2,
            y: p.play_area.y + p.play_area.height / 2,
          };
          damperRef.current.setTarget(center);
          posRef.current = center;
        }
        setScene(p.scene);
      }),
    );
    unlisteners.push(
      onPerformanceStopped(() => {
        setScene(null);
        applyPressed([]);
        resetMouseState();
        // 参数复位由 onParamFrame 的空帧分支执行（touched 集合）。
      }),
    );
    unlisteners.push(onDeviceChanged(handleDevice));

    void api.isPerforming().then((s) => {
      if (s) setScene(s);
    });

    return () => {
      for (const u of unlisteners) void u.then((fn) => fn());
    };
  }, [applyPressed, handleDevice, resetMouseState]);

  // 鼠标轨迹阻尼插值：仅 mouse 场景推进（高频，只写 ref）。
  // PIXI v6 Ticker.add 回调参数是帧 delta（number），阻尼需要毫秒级 deltaMS。
  useEffect(() => {
    const tick = () => {
      if (sceneRef.current !== "mouse") return;
      const pos = damperRef.current.update(Ticker.shared.deltaMS);
      if (pos) posRef.current = pos;
    };
    Ticker.shared.add(tick);
    return () => {
      Ticker.shared.remove(tick);
    };
  }, []);

  // 每帧参数直写：注册在 Live2dStage 的 afterMotionUpdate 时序。
  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    return stage.onParamFrame((writer) => {
      const current = sceneRef.current;
      if (!current) {
        // 表演已停/未开始：复位本引擎触碰过的参数（写回默认值一帧），随后 motion 接管。
        for (const id of touchedRef.current) {
          writer.reset(id);
        }
        touchedRef.current.clear();
        return;
      }
      // 键盘通道（typing / both）：手按下参数。
      const frame: ParamMapping[] = [];
      if (current === "typing" || current === "both") {
        const pressed = pressedRef.current;
        frame.push(
          ...handParams(
            pressed.some((p) => p.hand === "left"),
            pressed.some((p) => p.hand === "right"),
          ),
        );
      }
      // 鼠标通道（mouse / both）：光标跟随 + 鼠标键按下。两路参数无重叠，可合并写入。
      if (current === "mouse" || current === "both") {
        const area = playAreaRef.current;
        const pos = posRef.current;
        if (area && pos) {
          const xRatio = Math.max(0, Math.min(1, (pos.x - area.x) / area.width));
          const yRatio = Math.max(0, Math.min(1, (pos.y - area.y) / area.height));
          frame.push(...mapCursorToParams(xRatio, yRatio, writer.range));
        }
        frame.push(...mouseButtonParams(mouseRef.current.leftDown, mouseRef.current.rightDown));
      }
      for (const { id, value } of frame) {
        if (writer.set(id, value)) {
          touchedRef.current.add(id);
        }
      }
    });
  }, [stageRef]);

  return { scene, pressedKeys };
}
