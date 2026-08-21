/**
 * 光标轨迹阻尼插值：把高频目标点平滑成低频运动，避免参数抖动。
 *
 * 移植自 BongoCat `useDevice` 的 tickerCallback：每帧以
 * `alpha = 1 - 0.75^(deltaMS/16.7)` 向目标插值；总位移 < 0.5px 时 snap 到目标
 * 并返回收敛信号（引擎据此停止写入，光标停稳）。
 */
const DAMPING_DECAY = 0.75;

export interface Point2 {
  x: number;
  y: number;
}

export class Damper {
  private current: Point2 | null = null;
  private target: Point2 | null = null;

  /** 设置目标点；首次设置目标时以目标为起点（避免从 0,0 突跳）。 */
  setTarget(target: Point2): void {
    this.target = target;
    if (!this.current) {
      this.current = { ...target };
    }
  }

  /** 当前插值位置（无目标/未开始返回 null）。 */
  get value(): Point2 | null {
    return this.current;
  }

  /** 是否已收敛到当前目标。 */
  get settled(): boolean {
    if (!this.target || !this.current) {
      return true;
    }
    return Math.hypot(this.target.x - this.current.x, this.target.y - this.current.y) < 0.5;
  }

  /**
   * 按帧推进：返回最新插值位置；无目标返回 null。
   * 收敛时 snap 到目标并返回目标（下一次调用起 settled 为 true）。
   */
  update(deltaMS: number): Point2 | null {
    if (!this.target) {
      return null;
    }
    if (!this.current) {
      this.current = { ...this.target };
    }
    const alpha = 1 - DAMPING_DECAY ** (deltaMS / (1000 / 60));
    const x = this.current.x + (this.target.x - this.current.x) * alpha;
    const y = this.current.y + (this.target.y - this.current.y) * alpha;
    if (Math.hypot(this.target.x - x, this.target.y - y) < 0.5) {
      this.current = { ...this.target };
    } else {
      this.current = { x, y };
    }
    return this.current;
  }

  /** 清空目标与位置（表演停止/切换场景时用）。 */
  reset(): void {
    this.current = null;
    this.target = null;
  }
}
