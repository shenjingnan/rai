/** 量化展示与排序（纯函数）。 */
import { formatBytes } from "@/components/library/libraryMeta";

/** 量化显示名（原样，大写）。 */
export function quantLabel(q: string): string {
  return q.toUpperCase();
}

/** 量化质量评分（越大越好，用于排序展示；未知返回 null）。 */
export function quantQuality(q: string): number | null {
  const up = q.toUpperCase();
  const rank: Record<string, number> = {
    Q2_K: 1,
    Q3_K_S: 2,
    Q3_K_M: 3,
    Q4_0: 4,
    Q4_K_S: 5,
    Q4_K_M: 6,
    Q5_0: 7,
    Q5_K_S: 8,
    Q5_K_M: 9,
    Q6_K: 10,
    Q8_0: 11,
    Q8_K: 12,
    F16: 13,
    F32: 14,
  };
  return rank[up] ?? null;
}

/** 大文件字节格式化（复用 libraryMeta）。 */
export { formatBytes };

/**
 * 估算运行所需内存（GB，向上取整）。
 * 经验公式：文件大小 × 1.5（模型权重加载 + KV cache + 运行 overhead）。
 * 仅 LLM 有意义；标注"估算"。
 */
export function estimateRamGb(fileSizeBytes: number): number {
  const gb = fileSizeBytes / 1024 ** 3;
  return Math.max(1, Math.ceil(gb * 1.5));
}

/** 字节 → GB 文案。 */
export function gbText(bytes: number): string {
  return (bytes / 1024 ** 3).toFixed(1);
}
