/// TTS 合成线程（`SynthHandle`）。
///
/// sherpa-onnx 的 `OfflineTts::generate_with_config` 是同步阻塞（数秒级），因此把
/// 合成放到独立线程，避免卡住编排线程。单消费者保证句子顺序：`Synthesize` 串行
/// 处理、结果严格按提交序回传，编排线程按接收序 `append` 到播放器即天然保序。
///
/// `gen_id`（generation id）用于打断后的过期丢弃：每次进入新一轮生成 `+1`，
/// 编排线程只接受 `gen_id == current` 的结果；打断时 `cancel_all()` 置 cancel，
/// 当前句经 `synthesize_with_progress` 的进度回调返回 false 提前终止，待处理命令
/// 快速返回错误（不浪费算力）。
use crate::tts::TtsEngine;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

/// 发给合成线程的命令。
enum SynthCommand {
    Synthesize { text: String, gen_id: u64 },
    Shutdown,
}

/// 合成结果（成功或失败，均带 `gen_id` 供编排线程做过期丢弃）。
pub enum SynthResult {
    Done {
        gen_id: u64,
        samples: Vec<f32>,
        sample_rate: u32,
    },
    Error {
        gen_id: u64,
        message: String,
    },
}

/// 合成线程句柄。
pub struct SynthHandle {
    tx: Sender<SynthCommand>,
    rx: Receiver<SynthResult>,
    cancel: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl SynthHandle {
    /// 启动合成线程。`ref_wav` / `ref_text` / `speed` 为每句合成固定使用的参考音色参数。
    pub fn new(tts: TtsEngine, ref_wav: PathBuf, ref_text: String, speed: f32) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let sample_rate = tts.sample_rate() as u32;

        let join = std::thread::Builder::new()
            .name("voice-tts".to_string())
            .spawn(move || {
                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        SynthCommand::Synthesize { text, gen_id } => {
                            // cancel 置位时跳过合成（进度回调返回 false 也用于提前终止当前句）
                            if cancel_clone.load(Ordering::Relaxed) {
                                let _ = done_tx.send(SynthResult::Error {
                                    gen_id,
                                    message: "已取消".to_string(),
                                });
                                continue;
                            }
                            // progress 回调返回 false 可提前终止当前句（打断时减少无用计算）；
                            // 闭包需 'static，clone 一份 cancel 标志再 move。
                            let progress_cancel = cancel_clone.clone();
                            let result = tts.synthesize_with_progress(
                                &text,
                                speed,
                                &ref_wav,
                                &ref_text,
                                move |_p| !progress_cancel.load(Ordering::Relaxed),
                            );
                            let payload = match result {
                                Ok(samples) => SynthResult::Done {
                                    gen_id,
                                    samples,
                                    sample_rate,
                                },
                                Err(e) => SynthResult::Error { gen_id, message: e },
                            };
                            let _ = done_tx.send(payload);
                        }
                        SynthCommand::Shutdown => break,
                    }
                }
            })
            .expect("spawn voice-tts 线程失败");

        Self {
            tx: cmd_tx,
            rx: done_rx,
            cancel,
            join: Some(join),
        }
    }

    /// 入队一句文本合成（非阻塞）。
    pub fn enqueue(&self, text: String, gen_id: u64) {
        let _ = self.tx.send(SynthCommand::Synthesize { text, gen_id });
    }

    /// 非阻塞拉取一个合成结果（None = 暂无结果）。
    pub fn try_recv(&self) -> Option<SynthResult> {
        self.rx.try_recv().ok()
    }

    /// 取消当前一轮的合成（打断时调用）：置 cancel，让当前句提前终止、待处理跳过。
    pub fn cancel_all(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// 新一轮生成开始前复位取消标志。
    pub fn clear_cancel(&self) {
        self.cancel.store(false, Ordering::Relaxed);
    }
}

impl Drop for SynthHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(SynthCommand::Shutdown);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用一个「立即返回固定样本」的假合成（不依赖真实 TTS 模型）测试句柄的
    /// 队列/取消语义。真实 `TtsEngine` 需要模型文件，这里只测句柄逻辑。
    fn handle_with_fake_tts() -> (SynthHandle, Arc<AtomicBool>) {
        // 直接构造：跳过线程内的真实合成，模拟通过 enqueue + 手动构造结果较复杂。
        // 这里用哨兵：cancel 标志作为「是否执行」开关，无法注入假引擎时退化为
        // 结构/生命周期测试（见下）。真实队列语义在 session.rs 集成测试覆盖。
        let (cmd_tx, cmd_rx) = mpsc::channel::<SynthCommand>();
        let (done_tx, done_rx) = mpsc::channel::<SynthResult>();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let join = std::thread::spawn(move || {
            // 假合成：从文本里取字符数作为「合成耗时」，产出固定样本
            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    SynthCommand::Synthesize { text, gen_id } => {
                        if cancel_clone.load(Ordering::Relaxed) {
                            let _ = done_tx.send(SynthResult::Error {
                                gen_id,
                                message: "已取消".to_string(),
                            });
                            continue;
                        }
                        // 模拟耗时：句长 ms 级
                        std::thread::sleep(std::time::Duration::from_millis(
                            (text.chars().count() as u64).min(20),
                        ));
                        let _ = done_tx.send(SynthResult::Done {
                            gen_id,
                            samples: vec![0.0; 240],
                            sample_rate: 24000,
                        });
                    }
                    SynthCommand::Shutdown => break,
                }
            }
        });
        (
            SynthHandle {
                tx: cmd_tx,
                rx: done_rx,
                cancel: cancel.clone(),
                join: Some(join),
            },
            cancel,
        )
    }

    #[test]
    fn test_enqueue_and_recv_in_order() {
        let (h, _) = handle_with_fake_tts();
        h.enqueue("第一句".to_string(), 1);
        h.enqueue("第二句".to_string(), 1);

        // 等待两个结果按提交序返回
        let mut got = Vec::new();
        for _ in 0..2 {
            loop {
                if let Some(r) = h.try_recv() {
                    match r {
                        SynthResult::Done {
                            gen_id,
                            samples,
                            sample_rate,
                        } => {
                            got.push((gen_id, samples.len(), sample_rate));
                        }
                        SynthResult::Error { message, .. } => panic!("不应失败: {message}"),
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], (1, 240, 24000));
        assert_eq!(got[1], (1, 240, 24000));
    }

    #[test]
    fn test_cancel_all_returns_errors() {
        let (h, _) = handle_with_fake_tts();
        h.cancel_all();
        h.enqueue("一句话".to_string(), 7);
        let mut got_error = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if let Some(SynthResult::Error { gen_id, .. }) = h.try_recv() {
                assert_eq!(gen_id, 7);
                got_error = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(got_error, "cancel 后应快速返回错误");
    }

    #[test]
    fn test_clear_cancel_resumes() {
        let (h, _) = handle_with_fake_tts();
        h.cancel_all();
        h.clear_cancel();
        h.enqueue("恢复".to_string(), 3);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut done = false;
        while std::time::Instant::now() < deadline {
            if let Some(r) = h.try_recv() {
                assert!(matches!(r, SynthResult::Done { .. }));
                done = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(done, "clear_cancel 后应正常合成");
    }
}
