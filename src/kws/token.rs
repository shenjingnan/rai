//! 用户自定义关键词 → sherpa-onnx 可编码的 ppinyin token。
//!
//! 本项目模型（zipformer-zh-en）的 ppinyin 分词把每个汉字拆成「声母 + 韵母」：
//! `文` → `w` `én`，`索` → `s` `uǒ`。用户直接输入原始中文（如 `你好小智`）时，
//! 需要先转成这样的 token 序列，sherpa-onnx 才能编码；否则编码失败返回空指针流，
//! 后续喂音频会直接段错误。
//!
//! 用法：`encode_custom_keywords` 把用户输入（原始中文 / 已 tokenized 拼音 /
//! 带 `@` 显示词 / 多个关键词用 `/` 或换行分隔）统一编码成 sherpa 可接受的格式。
use std::collections::HashSet;
use std::path::Path;

use pinyin::ToPinyin;

/// 双字母声母（先匹配，避免 `zh` 被拆成 `z h`）。
const INITIALS_2: [&str; 3] = ["zh", "ch", "sh"];
/// 单字母声母（ppinyin 约定把 `y`/`w` 也当声母，如 `文` = `w én`）。
const INITIALS_1: [&str; 20] = [
    "b", "p", "m", "f", "d", "t", "n", "l", "g", "k", "h", "j", "q", "x", "r", "z", "c", "s", "y",
    "w",
];

/// 无标准声母/韵母拆分的整音节（整个作为 token）。
const SPECIAL_SYLLABLES: [&str; 6] = ["hm", "hng", "ń", "ň", "ḿ", "ǹ"];

/// 是否为 CJK 汉字（基本区 + 扩展 A + 兼容区）。
fn is_cjk(c: char) -> bool {
    matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{f900}'..='\u{faff}')
}

/// 读取 tokens.txt（每行 `token id`），返回 token 集合（取第一列）。
pub fn load_token_set(tokens_path: &Path) -> Result<HashSet<String>, String> {
    let content = std::fs::read_to_string(tokens_path)
        .map_err(|e| format!("无法读取 tokens.txt {}: {}", tokens_path.display(), e))?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_string)
        .collect())
}

/// 把一个带声调拼音音节（如 `nǐ`）拆成「声母 + 韵母」。
/// 返回 `(声母, 韵母)`；无声母时声母为空串。
fn split_syllable(syl: &str, tokens: &HashSet<String>) -> Result<(String, Option<String>), String> {
    if SPECIAL_SYLLABLES.contains(&syl) {
        return Ok((String::new(), Some(syl.to_string())));
    }
    for init in INITIALS_2 {
        if let Some(rest) = syl.strip_prefix(init)
            && !rest.is_empty()
            && tokens.contains(rest)
        {
            return Ok((init.to_string(), Some(rest.to_string())));
        }
    }
    for init in INITIALS_1 {
        if let Some(rest) = syl.strip_prefix(init)
            && !rest.is_empty()
            && tokens.contains(rest)
        {
            return Ok((init.to_string(), Some(rest.to_string())));
        }
    }
    // 无标准拆分：整个音节应是 tokens 中的韵母或特殊音节
    if tokens.contains(syl) {
        return Ok((String::new(), Some(syl.to_string())));
    }
    Err(format!(
        "无法把拼音 `{syl}` 拆分为模型 token（tokens.txt 中无匹配韵母）"
    ))
}

/// 把汉字文本转成 ppinyin token 序列。
///
/// 例：`你好小智` → `["n", "ǐ", "h", "ǎo", "x", "iǎo", "zh", "ì"]`
pub fn hanzi_to_ppinyin(text: &str, tokens: &HashSet<String>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for p in text.to_pinyin() {
        match p {
            Some(p) => {
                let (init, fin) = split_syllable(p.with_tone(), tokens)?;
                if !init.is_empty() {
                    out.push(init);
                }
                if let Some(f) = fin {
                    out.push(f);
                }
            }
            None => {
                // 非汉字（空格、标点）跳过
            }
        }
    }
    if out.is_empty() {
        return Err(format!("文本 `{text}` 中没有可转换的汉字"));
    }
    Ok(out)
}

/// 校验 token 序列中的每个 token 都在模型 tokens 中。
fn validate_tokens(token_str: &str, tokens: &HashSet<String>) -> Result<(), String> {
    for tok in token_str.split_whitespace() {
        if !tokens.contains(tok) {
            return Err(format!(
                "token `{tok}` 不在模型 tokens.txt 中。\n\
                 请使用模型支持的拼音/音素格式，或直接输入中文由程序自动转换。"
            ));
        }
    }
    Ok(())
}

/// 把用户输入的自定义关键词编码成 sherpa-onnx 可接受的格式。
///
/// 支持输入：
/// - 原始中文（自动转 ppinyin token，显示词取原文）：`你好小智`
/// - 已 tokenized 拼音：`n ǐ h ǎo x iǎo zh ì`
/// - 显式显示词：`n ǐ h ǎo x iǎo zh ì @你好小智`
/// - 多个关键词：用 `/` 或换行分隔
pub fn encode_custom_keywords(input: &str, tokens_path: &Path) -> Result<String, String> {
    let tokens = load_token_set(tokens_path)?;
    let mut lines = Vec::new();
    for raw in input.split(['/', '\n']) {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        lines.push(encode_keyword(raw, &tokens)?);
    }
    if lines.is_empty() {
        return Err("未提供任何关键词".to_string());
    }
    Ok(lines.join("\n"))
}

/// 编码单个关键词。
fn encode_keyword(raw: &str, tokens: &HashSet<String>) -> Result<String, String> {
    // 拆出 `@` 后的显式显示词（可选）
    let (token_part, display) = match raw.rsplit_once('@') {
        Some((t, d)) => (t.trim(), Some(d.trim().to_string())),
        None => (raw, None),
    };
    let token_part_has_cjk = token_part.chars().any(is_cjk);

    // 原始中文 → ppinyin token；否则按原样（已是 token 序列）
    let token_str = if token_part_has_cjk {
        hanzi_to_ppinyin(token_part, tokens)?.join(" ")
    } else {
        token_part.to_string()
    };

    validate_tokens(&token_str, tokens)?;

    // 显示词缺省时：原始中文作为显示词；纯拼音不附加
    let display = display.or_else(|| token_part_has_cjk.then(|| token_part.to_string()));
    match display {
        Some(d) => Ok(format!("{token_str} @{d}")),
        None => Ok(token_str),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_tokens(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    /// 真实模型 tokens.txt 的简化版（`token id` 两列），覆盖测试用到的拼音 token。
    fn real_tokens() -> tempfile::NamedTempFile {
        temp_tokens(
            "<blk> 0\nAA0 3\nB 21\nn 131\nǚ 259\nǐ 251\nh 88\nǎo 249\nx 179\niǎo 124\nzh 182\nì 203\nf 86\nǎ 245\ng 87\nuó 163\n",
        )
    }

    #[test]
    fn test_load_token_set_takes_first_column() {
        let f = real_tokens();
        let set = load_token_set(f.path()).unwrap();
        assert!(set.contains("n"));
        assert!(set.contains("ǚ"));
        assert!(!set.contains("n 131"), "应取第一列，不含 id");
    }

    #[test]
    fn test_hanzi_to_ppinyin_ni_hao_xiao_zhi() {
        let f = real_tokens();
        let tokens = load_token_set(f.path()).unwrap();
        let out = hanzi_to_ppinyin("你好小智", &tokens).unwrap();
        assert_eq!(out, vec!["n", "ǐ", "h", "ǎo", "x", "iǎo", "zh", "ì"]);
    }

    #[test]
    fn test_encode_raw_chinese() {
        let f = real_tokens();
        let encoded = encode_custom_keywords("你好小智", f.path()).unwrap();
        assert_eq!(encoded, "n ǐ h ǎo x iǎo zh ì @你好小智");
    }

    #[test]
    fn test_encode_tokenized_pinyin() {
        let f = real_tokens();
        let encoded = encode_custom_keywords("n ǐ h ǎo x iǎo zh ì", f.path()).unwrap();
        assert_eq!(encoded, "n ǐ h ǎo x iǎo zh ì");
    }

    #[test]
    fn test_encode_with_explicit_display() {
        let f = real_tokens();
        let encoded = encode_custom_keywords("n ǐ h ǎo x iǎo zh ì @测试", f.path()).unwrap();
        assert_eq!(encoded, "n ǐ h ǎo x iǎo zh ì @测试");
    }

    #[test]
    fn test_encode_multiple_keywords_slash_separated() {
        let f = real_tokens();
        let encoded = encode_custom_keywords("你好/法国", f.path()).unwrap();
        // 你好 = n ǐ h ǎo；法国 = f ǎ g uó
        assert_eq!(encoded, "n ǐ h ǎo @你好\nf ǎ g uó @法国");
    }

    #[test]
    fn test_encode_invalid_token_errors() {
        let f = real_tokens();
        let err = encode_custom_keywords("L AY1", f.path()).unwrap_err();
        assert!(err.contains("L"));
    }

    #[test]
    fn test_encode_empty_errors() {
        let f = real_tokens();
        assert!(encode_custom_keywords("", f.path()).is_err());
        assert!(encode_custom_keywords("  \n/ ", f.path()).is_err());
    }
}
