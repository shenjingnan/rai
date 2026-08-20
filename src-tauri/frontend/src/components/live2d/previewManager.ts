import * as PIXI from "pixi.js";
import { Live2DModel } from "pixi-live2d-display/cubism4";
import { computeModelBounds, layoutModel } from "./modelLayout";

// pixi-live2d-display 依赖全局 window.PIXI.Ticker 驱动模型动画（autoUpdate 走 Ticker.shared）。
// 设置窗口改用共享舞台后可能不再引入 Live2dStage，这里同样注入。
if (typeof window !== "undefined") {
  (window as unknown as { PIXI: typeof PIXI }).PIXI = PIXI;
}

export interface PreviewSlotCallbacks {
  /** 渲染初始化或模型加载失败时的回调。 */
  onError?: (error: Error) => void;
  /** 模型加载完成、可计算角色真实边界时回调。 */
  onModelMetrics?: (metrics: { aspectRatio: number }) => void;
  /** 模型上舞台、画布可用时回调（缓存命中也会触发，上层需自行去重）。 */
  onModelReady?: (canvas: HTMLCanvasElement) => void;
}

export interface ClaimOptions {
  /** 页面提供的占位元素（slot），canvas 会被移动到其内部。 */
  element: HTMLElement;
  width: number;
  height: number;
  modelScale: number;
  callbacks: PreviewSlotCallbacks;
}

export interface ClaimHandle {
  readonly id: number;
  /** 尺寸/缩放变化：只 resize 渲染器并重新布局，不重建、不重载。 */
  updateLayout(width: number, height: number, modelScale: number): void;
  /** 展示模型：null 清屏；同 url 同 reloadKey 命中缓存零加载；reloadKey 变化强制重载。 */
  showModel(modelUrl: string | null, reloadKey: string | number): void;
  /** 释放占用：仅当自己是当前占用者时生效（stale-safe，乱序 release 不影响新占用者）。 */
  release(): void;
}

/** 模型 LRU 缓存上限：概览/伙伴页最近使用的模型往返切换免加载。 */
const MAX_CACHED_MODELS = 3;

interface CacheEntry {
  model: Live2DModel;
  reloadKey: string | number;
}

class HandleImpl implements ClaimHandle {
  readonly id: number;
  element: HTMLElement;
  width: number;
  height: number;
  modelScale: number;
  callbacks: PreviewSlotCallbacks;
  /** 最近一次请求展示的 url：异步加载完成后校验是否仍是当前意图。 */
  requestedUrl: string | null = null;

  constructor(
    private manager: PreviewManager,
    id: number,
    opts: ClaimOptions,
  ) {
    this.id = id;
    this.element = opts.element;
    this.width = opts.width;
    this.height = opts.height;
    this.modelScale = opts.modelScale;
    this.callbacks = opts.callbacks;
  }

  updateLayout(width: number, height: number, modelScale: number): void {
    this.width = width;
    this.height = height;
    this.modelScale = modelScale;
    this.manager.layoutChanged(this);
  }

  showModel(modelUrl: string | null, reloadKey: string | number): void {
    this.requestedUrl = modelUrl;
    this.manager.showModel(this, modelUrl, reloadKey);
  }

  release(): void {
    this.manager.release(this);
  }
}

/**
 * 设置窗口的共享 Live2D 预览舞台（模块级单例，命令式）：
 *
 * - 整个窗口只有一个 PIXI.Application / WebGL 上下文，首次 claim 时懒创建；
 * - 页面切换只是把 canvas `appendChild` 移进新 slot（DOM 移动不丢 GL context），
 *   避免「同步销毁 + 同步重建 WebGL + 模型全量重载」压在切页 commit 里；
 * - 模型按 url 做 LRU 缓存，同模型往返零加载；reloadKey 变化强制重载
 *   （等价原 React key 重挂载语义，供概览页失败重试）；
 * - ticker 双通道治理：渲染循环用 app.ticker（claim/release 启停），模型动画
 *   走库硬编码的 Ticker.shared（autoUpdate），上下舞台必须同步翻转 per-model
 *   autoUpdate，否则停放进缓存的模型仍每帧空转。
 *
 * 桌宠窗口（companion.html 独立 WebView）不经过本类，继续用 Live2dStage。
 */
export class PreviewManager {
  private app: PIXI.Application | null = null;
  private parking: HTMLDivElement | null = null;
  private current: HandleImpl | null = null;
  private nextId = 1;
  private cache = new Map<string, CacheEntry>();
  private inflight = new Map<string, Promise<Live2DModel>>();
  private shownUrl: string | null = null;
  private shownModel: Live2DModel | null = null;
  private visibilityBound = false;

  claim(opts: ClaimOptions): ClaimHandle {
    const handle = new HandleImpl(this, this.nextId++, opts);
    this.current = handle;
    const app = this.ensureApp(handle);
    if (!app) {
      return handle;
    }
    app.view.remove(); // 已在其它 slot 时先脱离，再挂进新 slot
    handle.element.appendChild(app.view);
    app.renderer.resize(handle.width, handle.height);
    app.ticker.start();
    return handle;
  }

  release(handle: HandleImpl): void {
    if (this.current?.id !== handle.id) return;
    this.detachShown();
    if (this.app && this.parking) {
      this.parking.appendChild(this.app.view);
      this.app.ticker.stop();
    }
    this.current = null;
  }

  layoutChanged(handle: HandleImpl): void {
    if (this.current?.id !== handle.id || !this.app) return;
    this.app.renderer.resize(handle.width, handle.height);
    if (this.shownModel) {
      layoutModel(this.shownModel, handle.width, handle.height, handle.modelScale);
    }
  }

  showModel(handle: HandleImpl, url: string | null, reloadKey: string | number): void {
    if (this.current?.id !== handle.id) return;

    if (url === null) {
      this.detachShown();
      return;
    }

    const cached = this.cache.get(url);
    if (cached && cached.reloadKey === reloadKey) {
      if (this.shownUrl === url) return; // 已在显示，no-op
      this.cache.delete(url);
      this.cache.set(url, cached); // LRU 触新
      this.attach(handle, url, cached);
      return;
    }

    if (cached) {
      // reloadKey 变化（如概览页失败重试）：销毁旧实例强制重载。
      this.cache.delete(url);
      if (this.shownUrl === url) {
        this.detachShown();
      }
      cached.model.destroy();
    } else {
      this.detachShown(); // 换模型：旧模型先下舞台（留在缓存）
    }

    const promise =
      this.inflight.get(url) ??
      Live2DModel.from(url, { autoInteract: false }).then((model) => {
        this.cache.set(url, { model, reloadKey });
        this.evict();
        this.inflight.delete(url);
        return model;
      });
    this.inflight.set(url, promise);

    void promise.then(
      (model) => {
        if (this.current?.id === handle.id && handle.requestedUrl === url) {
          this.attach(handle, url, { model, reloadKey });
        }
        // 已被其它 handle 接管：模型留在缓存，下个占用者免加载。
      },
      (e) => {
        console.error("Live2D 模型加载失败:", e);
        this.inflight.delete(url);
        if (this.current?.id === handle.id && handle.requestedUrl === url) {
          handle.callbacks.onError?.(e instanceof Error ? e : new Error(String(e)));
        }
      },
    );
  }

  /** 懒创建共享 app：已存在直接返回，创建失败回调 onError 并返回 null（下次 claim 重试）。 */
  private ensureApp(handle: HandleImpl): PIXI.Application | null {
    if (this.app) return this.app;
    try {
      const app = new PIXI.Application({
        width: handle.width,
        height: handle.height,
        backgroundAlpha: 0,
        antialias: true,
        autoStart: false,
        resolution: window.devicePixelRatio || 1,
        autoDensity: true,
      });
      app.view.style.display = "block";
      // 离屏停放容器：无页面占用时挂在这里（防 GC，也避免误渲染）。
      const parking = document.createElement("div");
      parking.style.display = "none";
      parking.appendChild(app.view);
      document.body.appendChild(parking);
      this.app = app;
      this.parking = parking;
      this.bindVisibility();
      return app;
    } catch (e) {
      console.error("PIXI 初始化失败:", e);
      handle.callbacks.onError?.(e instanceof Error ? e : new Error(String(e)));
      return null;
    }
  }

  /** 模型上舞台：加进 stage、开启动画、按当前 slot 尺寸布局并回调。 */
  private attach(handle: HandleImpl, url: string, entry: CacheEntry): void {
    const app = this.app;
    if (!app) return;
    this.detachShown();
    app.stage.addChild(entry.model);
    entry.model.autoUpdate = true;
    this.shownUrl = url;
    this.shownModel = entry.model;
    layoutModel(entry.model, handle.width, handle.height, handle.modelScale);
    const bounds = computeModelBounds(entry.model);
    const valid =
      Number.isFinite(bounds.width) &&
      Number.isFinite(bounds.height) &&
      bounds.width > 0 &&
      bounds.height > 0;
    if (valid) {
      handle.callbacks.onModelMetrics?.({ aspectRatio: bounds.width / bounds.height });
    }
    handle.callbacks.onModelReady?.(app.view as HTMLCanvasElement);
  }

  private detachShown(): void {
    const model = this.shownModel;
    if (!model) return;
    this.app?.stage.removeChild(model);
    model.autoUpdate = false; // 模型动画走 Ticker.shared，与 app.ticker 无关，须显式关
    this.shownModel = null;
    this.shownUrl = null;
  }

  /** 超出容量时逐出最旧的非当前显示模型（当前显示的刚被触新，天然在末尾）。 */
  private evict(): void {
    while (this.cache.size > MAX_CACHED_MODELS) {
      let evicted: [string, CacheEntry] | null = null;
      for (const entry of this.cache) {
        if (entry[0] === this.shownUrl) continue;
        evicted = entry;
        break;
      }
      if (!evicted) break;
      this.cache.delete(evicted[0]);
      evicted[1].model.destroy();
    }
  }

  /** 窗口隐藏时停摆（ticker + 模型动画），可见时若有占用者则恢复。 */
  private bindVisibility(): void {
    if (this.visibilityBound) return;
    this.visibilityBound = true;
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) {
        this.app?.ticker.stop();
        if (this.shownModel) this.shownModel.autoUpdate = false;
      } else if (this.current && this.app) {
        this.app.ticker.start();
        if (this.shownModel) this.shownModel.autoUpdate = true;
      }
    });
  }
}

let instance: PreviewManager | null = null;

/** 获取设置窗口共享预览舞台的单例。 */
export function getPreviewManager(): PreviewManager {
  instance ??= new PreviewManager();
  return instance;
}
