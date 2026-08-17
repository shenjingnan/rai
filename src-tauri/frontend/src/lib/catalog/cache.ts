/** 前端轻量 TTL 缓存（列表/detail/files 的兜底；Rust 侧已有主缓存）。 */
export interface CacheEntry<T> {
  value: T;
  at: number;
}

export class TtlCache<T> {
  private map = new Map<string, CacheEntry<T>>();
  private readonly ttlMs: number;
  private readonly maxEntries: number;

  constructor(ttlMs: number, maxEntries = 30) {
    this.ttlMs = ttlMs;
    this.maxEntries = maxEntries;
  }

  get(key: string): T | null {
    const entry = this.map.get(key);
    if (!entry) return null;
    if (Date.now() - entry.at > this.ttlMs) {
      this.map.delete(key);
      return null;
    }
    return entry.value;
  }

  set(key: string, value: T): void {
    if (this.map.size >= this.maxEntries && !this.map.has(key)) {
      // FIFO：删除最早的一条
      const oldest = this.map.keys().next().value as string | undefined;
      if (oldest) this.map.delete(oldest);
    }
    this.map.set(key, { value, at: Date.now() });
  }

  clear(): void {
    this.map.clear();
  }
}
