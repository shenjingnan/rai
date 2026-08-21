//! 种子伪随机数生成器（xorshift64*）。
//!
//! 零外部依赖、确定性可测：同一种子产生完全相同的序列，供表演模拟器的事件流
//! 性质测试稳定断言。xorshift64* 对表演模拟用途足够均匀，无需密码学随机。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 熵种子来源的计数器（与时间戳混合，避免多次构造得到相同种子）。
static ENTROPY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 轻量 PRNG（非加密随机）。
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// 以确定性种子构造。种子 `| 1` 保证非零状态（xorshift 全零会卡死）。
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// 以「时间戳 + 计数器」构造不可预测的种子。
    pub fn from_entropy() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let counter = ENTROPY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mix = counter.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        Self::new((nanos as u64) ^ mix)
    }

    /// 下一个 u64（xorshift64*）。
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// `[0, 1)` 的 f64（53 位尾数均匀分布）。
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// `[min, max]` 闭区间的 f64。
    pub fn range(&mut self, min: f64, max: f64) -> f64 {
        debug_assert!(max >= min);
        min + (max - min) * self.next_f64()
    }

    /// 随机挑选一个元素（空切片会 panic，调用方需保证非空）。
    pub fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        assert!(!slice.is_empty(), "Rng::pick 不能用于空切片");
        &slice[(self.next_u64() % slice.len() as u64) as usize]
    }

    /// 概率事件：以 `p` 概率返回 true。
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// `[min, max]` 闭区间均匀整数。
    pub fn int_range(&mut self, min: u64, max: u64) -> u64 {
        debug_assert!(max >= min);
        min + (self.next_u64() % (max - min + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_f64(), b.next_f64());
        }
    }

    #[test]
    fn next_f64_in_unit_interval() {
        let mut rng = Rng::new(7);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn range_respects_bounds() {
        let mut rng = Rng::new(9);
        for _ in 0..1000 {
            let v = rng.range(-2.0, 5.0);
            assert!((-2.0..=5.0).contains(&v));
        }
    }

    #[test]
    fn pick_within_slice() {
        let items = ["a", "b", "c"];
        let mut rng = Rng::new(11);
        for _ in 0..100 {
            let picked = rng.pick(&items);
            assert!(items.contains(picked));
        }
    }

    #[test]
    #[should_panic(expected = "空切片")]
    fn pick_empty_slice_panics() {
        let empty: [u8; 0] = [];
        Rng::new(1).pick(&empty);
    }

    #[test]
    fn int_range_respects_bounds() {
        let mut rng = Rng::new(13);
        for _ in 0..1000 {
            let v = rng.int_range(3, 10);
            assert!((3..=10).contains(&v));
        }
    }

    #[test]
    fn entropy_seeds_differ() {
        let a = Rng::from_entropy().next_f64();
        let b = Rng::from_entropy().next_f64();
        assert!(a != b, "两次熵种子应产生不同序列");
    }
}
