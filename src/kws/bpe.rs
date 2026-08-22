//! gigaspeech KWS 模型的英文关键词 → 子词 token（内置最小 unigram 分词器）。
//!
//! 模型（zipformer-gigaspeech）的 tokens.txt 是 500 个子词（`▁HE`、`LL`、`O`…），
//! 英文唤醒词需先用包内 `bpe.model` 切分成子词才能编码，否则会产生无效 token 流。
//! 词表以全大写为主（GigaSpeech 语料为大写文本），编码前统一大写化
//! （`hey momo` 与 `HEY MOMO` 编码结果一致）。
//!
//! **为什么手写而不用 sentencepiece crate**：该 crate 全版本依赖 `sentencepiece-sys`
//! （C++/cmake 构建），其静态链接的 protobuf 与 `sherpa-onnx-sys` 内嵌的 protobuf 重复
//! 符号（ODR 冲突），同链即段错误（已实测）。故此处内置最小实现：
//!
//! - 解析 ModelProto protobuf（仅 NORMAL 类型 pieces 的字符串与对数概率分）；
//! - 该文件虽名为 bpe.model，`trainer_spec.model_type` 实为 **UNIGRAM**（已解析核实，
//!   sherpa 沿用 icefall 的 `lang_bpe_500` 命名），编码即 unigram Viterbi：
//!   最大化子词对数概率之和，未知字符按 `min_score - 10` 回退（官方 kUnkPenalty 约定），
//!   连续未知段合并为一个表面 piece；
//! - 输出向量已与官方 sentencepiece（C++ 0.11/0.14）逐一比对一致。
//!
//! 探测方式与 zh-en 的 `en.phone` 发音词典一致：`bpe.model` 与 tokens.txt 同在模型
//! 根目录即视为该模式；zh-en / wenetspeech 包内无此文件，不受影响。
//! 产出的 piece 是否在模型 tokens.txt 中由 [`super::token`] 的校验逻辑把关。

use std::collections::HashMap;
use std::path::Path;

/// 从 bpe.model 加载的 unigram 分词模型（NORMAL pieces + 对数概率分）。
#[derive(Debug)]
pub struct BpeModel {
    vocab: HashMap<String, f32>,
    /// 未知字符回退分：词表最小分 - 10（官方 sentencepiece 的 kUnkPenalty 约定）。
    unk_score: f32,
    /// 词表中最长 piece 的字节数（Viterbi 回看窗口）。
    max_piece_len: usize,
}

/// 加载 bpe.model（gigaspeech 包自带，与 tokens.txt 同目录）。
pub fn load(model_path: &Path) -> Result<BpeModel, String> {
    let buf = std::fs::read(model_path)
        .map_err(|e| format!("无法读取 BPE 模型 {}: {}", model_path.display(), e))?;
    let mut vocab: HashMap<String, f32> = HashMap::new();
    let mut r = ProtoReader::new(&buf);
    while !r.eof() {
        let (field, wire) = r.tag().map_err(|e| format!("bpe.model 解析失败: {e}"))?;
        // ModelProto 字段 1 = repeated SentencePiece；其余（trainer/normalizer spec）跳过
        if field == 1 && wire == 2 {
            let len = r.varint().map_err(|e| format!("bpe.model 解析失败: {e}"))? as usize;
            let piece_bytes = r.bytes(len)?;
            let mut pr = ProtoReader::new(piece_bytes);
            let mut piece = String::new();
            let mut score = 0f32;
            let mut ptype = 1u64; // SentencePiece.Type 默认 NORMAL
            while !pr.eof() {
                let (pf, pw) = pr.tag().map_err(|e| format!("bpe.model 解析失败: {e}"))?;
                match (pf, pw) {
                    (1, 2) => {
                        let l = pr.varint()? as usize;
                        piece = String::from_utf8_lossy(pr.bytes(l)?).into_owned();
                    }
                    (2, 5) => score = f32::from_bits(pr.fixed32()?),
                    (3, 0) => ptype = pr.varint()?,
                    _ => pr.skip(pw)?,
                }
            }
            if ptype == 1 {
                vocab.insert(piece, score);
            }
        } else {
            r.skip(wire)
                .map_err(|e| format!("bpe.model 解析失败: {e}"))?;
        }
    }
    if vocab.is_empty() {
        return Err(format!("BPE 模型 {} 中无可用 pieces", model_path.display()));
    }
    let unk_score = vocab.values().copied().fold(f32::INFINITY, f32::min) - 10.0;
    let max_piece_len = vocab.keys().map(|k| k.len()).max().unwrap_or(1);
    Ok(BpeModel {
        vocab,
        unk_score,
        max_piece_len,
    })
}

/// 英文短语 → 子词列表（空格分隔前）。
///
/// 大写化后走 unigram Viterbi（最大化子词对数概率之和）。含词表外字符（标点/生僻组合）
/// 的输入会切出不在 tokens.txt 的 piece，由调用方校验并报清晰错误——无效 token 直接
/// 透传给 sherpa 会触发编码空指针崩溃。
pub fn encode_phrase(model: &BpeModel, phrase: &str) -> Result<Vec<String>, String> {
    let upper = phrase.trim().to_uppercase();
    if upper.is_empty() {
        return Err("关键词为空".to_string());
    }
    Ok(viterbi_encode(model, &upper))
}

/// unigram Viterbi：`▁` 前缀 + 空格转 `▁` 后，在字符边界上求最优切分。
fn viterbi_encode(model: &BpeModel, input: &str) -> Vec<String> {
    let normalized = format!("▁{}", input.replace(' ', "▁"));
    let bytes = normalized.as_bytes();
    // 字符边界字节偏移（piece 切点只可能落在字符边界）
    let bounds: Vec<usize> = normalized
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(bytes.len()))
        .collect();
    let nb = bounds.len();
    let neg = f32::NEG_INFINITY;
    let mut best = vec![neg; nb];
    let mut prev = vec![usize::MAX; nb]; // 前驱边界下标
    let mut via_unk = vec![false; nb];
    best[0] = 0.0;

    for k in 1..nb {
        let j = bounds[k];
        let earliest = j.saturating_sub(model.max_piece_len);
        for m in (0..k).rev() {
            let i = bounds[m];
            if i < earliest {
                break;
            }
            if best[m] == neg {
                continue;
            }
            if let Ok(s) = std::str::from_utf8(&bytes[i..j])
                && let Some(&score) = model.vocab.get(s)
                && best[m] + score > best[k]
            {
                best[k] = best[m] + score;
                prev[k] = m;
                via_unk[k] = false;
            }
        }
        // 未知单字符回退（保证 DP 可达）；连续未知段在回填时合并为一个表面 piece
        if best[k - 1] != neg {
            let ch = &normalized[bounds[k - 1]..j];
            if !model.vocab.contains_key(ch) && best[k - 1] + model.unk_score > best[k] {
                best[k] = best[k - 1] + model.unk_score;
                prev[k] = k - 1;
                via_unk[k] = true;
            }
        }
    }

    // 回填
    let mut spans: Vec<(usize, usize, bool)> = Vec::new(); // (起点边界, 终点边界, unk)
    let mut k = nb - 1;
    while k > 0 {
        let p = prev[k];
        let unk = via_unk[k];
        match spans.last_mut() {
            Some(last) if last.2 && unk && last.0 == k => last.0 = p,
            _ => spans.push((p, k, unk)),
        }
        k = p;
    }
    spans
        .iter()
        .rev()
        .map(|&(s, e, _)| normalized[bounds[s]..bounds[e]].to_string())
        .collect()
}

/// 最小 protobuf wire format 读取器（varint / length-delimited / fixed32）。
struct ProtoReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ProtoReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn varint(&mut self) -> Result<u64, String> {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let Some(&b) = self.buf.get(self.pos) else {
                return Err("varint 越界".to_string());
            };
            self.pos += 1;
            result |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(len).ok_or("length 越界")?;
        if end > self.buf.len() {
            return Err("bytes 越界".to_string());
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn fixed32(&mut self) -> Result<u32, String> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// 下一个字段 tag：返回 (field_number, wire_type)。
    fn tag(&mut self) -> Result<(u64, u64), String> {
        let t = self.varint()?;
        Ok((t >> 3, t & 0x7))
    }

    fn skip(&mut self, wire: u64) -> Result<(), String> {
        match wire {
            0 => {
                self.varint()?;
            }
            1 => {
                self.bytes(8)?;
            }
            2 => {
                let len = self.varint()? as usize;
                self.bytes(len)?;
            }
            5 => {
                self.bytes(4)?;
            }
            _ => return Err(format!("不支持的 wire type {wire}")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 官方 gigaspeech 包内 bpe.model（239KB，Apache-2.0），钉住真实编码行为。
    /// 期望值与官方 sentencepiece（C++ 0.11/0.14）实测输出逐一比对一致。
    const BPE_MODEL: &[u8] = include_bytes!("testdata/bpe.model");

    fn write_model(dir: &Path) -> std::path::PathBuf {
        let p = dir.join("bpe.model");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(BPE_MODEL).unwrap();
        p
    }

    #[test]
    fn test_encode_matches_official_sentencepiece() {
        let dir = tempfile::tempdir().unwrap();
        let m = load(&write_model(dir.path())).unwrap();
        assert_eq!(
            encode_phrase(&m, "HELLO WORLD").unwrap(),
            vec!["▁HE", "LL", "O", "▁WORLD"]
        );
        assert_eq!(
            encode_phrase(&m, "HEY MOMO").unwrap(),
            vec!["▁HE", "Y", "▁MO", "MO"]
        );
        assert_eq!(
            encode_phrase(&m, "ALEXA").unwrap(),
            vec!["▁A", "LE", "X", "A"]
        );
        assert_eq!(
            encode_phrase(&m, "LOVE AND PEACE").unwrap(),
            vec!["▁LOVE", "▁AND", "▁P", "E", "A", "CE"]
        );
        // 词表外标点：切出表面字符 piece（由调用方对 tokens.txt 校验报错）
        assert_eq!(
            encode_phrase(&m, "HEY, MOMO").unwrap(),
            vec!["▁HE", "Y", ",", "▁MO", "MO"]
        );
    }

    #[test]
    fn test_encode_uppercases_input() {
        let dir = tempfile::tempdir().unwrap();
        let m = load(&write_model(dir.path())).unwrap();
        // 我们先大写化再编码（词表以全大写为主）：大小写输入得到同一规范切分。
        // 注：官方 sentencepiece 不做大写化，小写会按词表中的小写 piece 切分
        // （`hey momo` → `▁ hey ▁ momo`），此处是有意偏离，保证唤醒词大小写无关。
        assert_eq!(
            encode_phrase(&m, "hey momo").unwrap(),
            encode_phrase(&m, "HEY MOMO").unwrap()
        );
        assert_eq!(
            encode_phrase(&m, "hey momo").unwrap(),
            vec!["▁HE", "Y", "▁MO", "MO"]
        );
        assert_eq!(
            encode_phrase(&m, "HeY mOmO").unwrap(),
            vec!["▁HE", "Y", "▁MO", "MO"]
        );
    }

    #[test]
    fn test_encode_unknown_chars_merge_into_surface_run() {
        let dir = tempfile::tempdir().unwrap();
        let m = load(&write_model(dir.path())).unwrap();
        // 连续未知字符（CJK）合并为一个表面 piece，▁ 单独成词（与官方行为一致）
        assert_eq!(encode_phrase(&m, "你好").unwrap(), vec!["▁", "你好"]);
    }

    #[test]
    fn test_encode_empty_phrase_errors() {
        let dir = tempfile::tempdir().unwrap();
        let m = load(&write_model(dir.path())).unwrap();
        assert!(encode_phrase(&m, "  ").is_err());
    }

    #[test]
    fn test_load_missing_file_errors() {
        let err = load(Path::new("/nonexistent/bpe.model")).unwrap_err();
        assert!(err.contains("无法读取"));
    }
}
