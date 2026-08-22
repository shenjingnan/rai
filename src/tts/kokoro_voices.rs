//! Kokoro v1.1（kokoro-multi-lang-v1_1）的 103 音色静态表。
//!
//! `voices.bin` 是纯 float 嵌入数组（不含音色名），名字在 `model.onnx` 的 metadata
//! 里而 sherpa-onnx Rust API 不暴露，因此名字→sid 映射内置本表。
//!
//! 顺序权威来源：sherpa-onnx `scripts/kokoro/v1.1-zh/generate_voices_bin.py`
//! （sid 0-2 英文女声，随后 zf_001..099 / zm_009..100 按编号升序取存在者；
//! 编号不连续——上游只发布了 103 个音色文件，见 HF hexgrad/Kokoro-82M-v1.1-zh）。
use serde::Serialize;

/// 音色语言分组（serde snake_case，前端下拉分组键）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KokoroVoiceGroup {
    EnglishFemale,
    ChineseFemale,
    ChineseMale,
}

impl KokoroVoiceGroup {
    /// 分组的中文展示名。
    pub fn label(&self) -> &'static str {
        match self {
            Self::EnglishFemale => "英文女声",
            Self::ChineseFemale => "中文女声",
            Self::ChineseMale => "中文男声",
        }
    }
}

/// 一个 Kokoro 预置音色。
#[derive(Debug, Clone, Serialize)]
pub struct KokoroVoice {
    /// 音色 id（同时是持久化到 `[tts].voice` 的键，如 `zf_001`）。
    pub id: &'static str,
    /// 展示名（与 id 相同；分组信息见 `group`）。
    pub name: &'static str,
    /// speaker id（sherpa `GenerationConfig.sid`，0..=102）。
    pub sid: i32,
    /// 语言/性别分组。
    pub group: KokoroVoiceGroup,
}

/// v1.1 全部 103 音色，按 sid 升序。
pub static KOKORO_VOICES: [KokoroVoice; 103] = [
    KokoroVoice {
        id: "af_maple",
        name: "af_maple",
        sid: 0,
        group: KokoroVoiceGroup::EnglishFemale,
    },
    KokoroVoice {
        id: "af_sol",
        name: "af_sol",
        sid: 1,
        group: KokoroVoiceGroup::EnglishFemale,
    },
    KokoroVoice {
        id: "bf_vale",
        name: "bf_vale",
        sid: 2,
        group: KokoroVoiceGroup::EnglishFemale,
    },
    KokoroVoice {
        id: "zf_001",
        name: "zf_001",
        sid: 3,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_002",
        name: "zf_002",
        sid: 4,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_003",
        name: "zf_003",
        sid: 5,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_004",
        name: "zf_004",
        sid: 6,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_005",
        name: "zf_005",
        sid: 7,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_006",
        name: "zf_006",
        sid: 8,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_007",
        name: "zf_007",
        sid: 9,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_008",
        name: "zf_008",
        sid: 10,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_017",
        name: "zf_017",
        sid: 11,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_018",
        name: "zf_018",
        sid: 12,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_019",
        name: "zf_019",
        sid: 13,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_021",
        name: "zf_021",
        sid: 14,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_022",
        name: "zf_022",
        sid: 15,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_023",
        name: "zf_023",
        sid: 16,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_024",
        name: "zf_024",
        sid: 17,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_026",
        name: "zf_026",
        sid: 18,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_027",
        name: "zf_027",
        sid: 19,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_028",
        name: "zf_028",
        sid: 20,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_032",
        name: "zf_032",
        sid: 21,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_036",
        name: "zf_036",
        sid: 22,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_038",
        name: "zf_038",
        sid: 23,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_039",
        name: "zf_039",
        sid: 24,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_040",
        name: "zf_040",
        sid: 25,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_042",
        name: "zf_042",
        sid: 26,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_043",
        name: "zf_043",
        sid: 27,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_044",
        name: "zf_044",
        sid: 28,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_046",
        name: "zf_046",
        sid: 29,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_047",
        name: "zf_047",
        sid: 30,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_048",
        name: "zf_048",
        sid: 31,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_049",
        name: "zf_049",
        sid: 32,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_051",
        name: "zf_051",
        sid: 33,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_059",
        name: "zf_059",
        sid: 34,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_060",
        name: "zf_060",
        sid: 35,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_067",
        name: "zf_067",
        sid: 36,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_070",
        name: "zf_070",
        sid: 37,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_071",
        name: "zf_071",
        sid: 38,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_072",
        name: "zf_072",
        sid: 39,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_073",
        name: "zf_073",
        sid: 40,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_074",
        name: "zf_074",
        sid: 41,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_075",
        name: "zf_075",
        sid: 42,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_076",
        name: "zf_076",
        sid: 43,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_077",
        name: "zf_077",
        sid: 44,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_078",
        name: "zf_078",
        sid: 45,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_079",
        name: "zf_079",
        sid: 46,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_083",
        name: "zf_083",
        sid: 47,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_084",
        name: "zf_084",
        sid: 48,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_085",
        name: "zf_085",
        sid: 49,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_086",
        name: "zf_086",
        sid: 50,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_087",
        name: "zf_087",
        sid: 51,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_088",
        name: "zf_088",
        sid: 52,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_090",
        name: "zf_090",
        sid: 53,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_092",
        name: "zf_092",
        sid: 54,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_093",
        name: "zf_093",
        sid: 55,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_094",
        name: "zf_094",
        sid: 56,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zf_099",
        name: "zf_099",
        sid: 57,
        group: KokoroVoiceGroup::ChineseFemale,
    },
    KokoroVoice {
        id: "zm_009",
        name: "zm_009",
        sid: 58,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_010",
        name: "zm_010",
        sid: 59,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_011",
        name: "zm_011",
        sid: 60,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_012",
        name: "zm_012",
        sid: 61,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_013",
        name: "zm_013",
        sid: 62,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_014",
        name: "zm_014",
        sid: 63,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_015",
        name: "zm_015",
        sid: 64,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_016",
        name: "zm_016",
        sid: 65,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_020",
        name: "zm_020",
        sid: 66,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_025",
        name: "zm_025",
        sid: 67,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_029",
        name: "zm_029",
        sid: 68,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_030",
        name: "zm_030",
        sid: 69,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_031",
        name: "zm_031",
        sid: 70,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_033",
        name: "zm_033",
        sid: 71,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_034",
        name: "zm_034",
        sid: 72,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_035",
        name: "zm_035",
        sid: 73,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_037",
        name: "zm_037",
        sid: 74,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_041",
        name: "zm_041",
        sid: 75,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_045",
        name: "zm_045",
        sid: 76,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_050",
        name: "zm_050",
        sid: 77,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_052",
        name: "zm_052",
        sid: 78,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_053",
        name: "zm_053",
        sid: 79,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_054",
        name: "zm_054",
        sid: 80,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_055",
        name: "zm_055",
        sid: 81,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_056",
        name: "zm_056",
        sid: 82,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_057",
        name: "zm_057",
        sid: 83,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_058",
        name: "zm_058",
        sid: 84,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_061",
        name: "zm_061",
        sid: 85,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_062",
        name: "zm_062",
        sid: 86,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_063",
        name: "zm_063",
        sid: 87,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_064",
        name: "zm_064",
        sid: 88,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_065",
        name: "zm_065",
        sid: 89,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_066",
        name: "zm_066",
        sid: 90,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_068",
        name: "zm_068",
        sid: 91,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_069",
        name: "zm_069",
        sid: 92,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_080",
        name: "zm_080",
        sid: 93,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_081",
        name: "zm_081",
        sid: 94,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_082",
        name: "zm_082",
        sid: 95,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_089",
        name: "zm_089",
        sid: 96,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_091",
        name: "zm_091",
        sid: 97,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_095",
        name: "zm_095",
        sid: 98,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_096",
        name: "zm_096",
        sid: 99,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_097",
        name: "zm_097",
        sid: 100,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_098",
        name: "zm_098",
        sid: 101,
        group: KokoroVoiceGroup::ChineseMale,
    },
    KokoroVoice {
        id: "zm_100",
        name: "zm_100",
        sid: 102,
        group: KokoroVoiceGroup::ChineseMale,
    },
];

/// 最大合法 sid（v1.1 共 103 个音色，0..=102）。
pub const KOKORO_MAX_SID: i32 = 102;
/// 默认推荐音色：中文女声 zf_001（sid 3）。
pub const KOKORO_DEFAULT_SID: i32 = 3;

/// 全部音色（按 sid 升序）。
pub fn list_voices() -> &'static [KokoroVoice] {
    &KOKORO_VOICES
}

/// 音色名 → sid。精确匹配 id（如 `zf_001`）；纯数字串按 sid 解析后钳界。
pub fn sid_by_name(name: &str) -> Option<i32> {
    let name = name.trim();
    if let Some(v) = KOKORO_VOICES.iter().find(|v| v.id == name) {
        return Some(v.sid);
    }
    name.parse::<i32>()
        .ok()
        .map(normalize_sid)
        .filter(|s| (0..=KOKORO_MAX_SID).contains(s))
}

/// sid 钳界：越界（含负数）回落默认音色。
pub fn normalize_sid(sid: i32) -> i32 {
    if (0..=KOKORO_MAX_SID).contains(&sid) {
        sid
    } else {
        KOKORO_DEFAULT_SID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_completeness() {
        assert_eq!(KOKORO_VOICES.len(), 103);
        // sid 连续覆盖 0..=102 且升序（等于数组下标）
        for (i, v) in KOKORO_VOICES.iter().enumerate() {
            assert_eq!(v.sid, i as i32, "sid 应等于数组下标");
        }
        // id 唯一
        let mut ids: Vec<&str> = KOKORO_VOICES.iter().map(|v| v.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), KOKORO_VOICES.len(), "音色 id 必须唯一");
        // 分组区间与官方导出脚本一致：0-2 英文女声、3-57 中文女声、58-102 中文男声
        assert_eq!(KOKORO_VOICES[0].id, "af_maple");
        assert_eq!(KOKORO_VOICES[1].id, "af_sol");
        assert_eq!(KOKORO_VOICES[2].id, "bf_vale");
        assert_eq!(KOKORO_VOICES[3].id, "zf_001");
        assert_eq!(KOKORO_VOICES[57].id, "zf_099");
        assert_eq!(KOKORO_VOICES[58].id, "zm_009");
        assert_eq!(KOKORO_VOICES[102].id, "zm_100");
        assert!(
            KOKORO_VOICES
                .iter()
                .take(3)
                .all(|v| v.group == KokoroVoiceGroup::EnglishFemale)
        );
        assert!(
            KOKORO_VOICES
                .iter()
                .skip(3)
                .take(55)
                .all(|v| v.group == KokoroVoiceGroup::ChineseFemale)
        );
        assert!(
            KOKORO_VOICES
                .iter()
                .skip(58)
                .all(|v| v.group == KokoroVoiceGroup::ChineseMale)
        );
    }

    #[test]
    fn test_sid_by_name() {
        assert_eq!(sid_by_name("zf_001"), Some(3));
        assert_eq!(sid_by_name("zf_099"), Some(57));
        assert_eq!(sid_by_name("zm_009"), Some(58));
        assert_eq!(sid_by_name("zm_100"), Some(102));
        assert_eq!(sid_by_name("af_maple"), Some(0));
        // 纯数字串按 sid 解析
        assert_eq!(sid_by_name("7"), Some(7));
        assert_eq!(sid_by_name("102"), Some(102));
        // 越界数字回落默认（normalize_sid）
        assert_eq!(sid_by_name("999"), Some(KOKORO_DEFAULT_SID));
        // 未知名
        assert_eq!(sid_by_name("不存在的音色"), None);
        assert_eq!(sid_by_name(""), None);
    }

    #[test]
    fn test_normalize_sid() {
        assert_eq!(normalize_sid(0), 0);
        assert_eq!(normalize_sid(102), 102);
        assert_eq!(normalize_sid(103), KOKORO_DEFAULT_SID);
        assert_eq!(normalize_sid(-1), KOKORO_DEFAULT_SID);
        assert!((0..=KOKORO_MAX_SID).contains(&KOKORO_DEFAULT_SID));
    }

    #[test]
    fn test_group_label() {
        assert_eq!(KokoroVoiceGroup::ChineseFemale.label(), "中文女声");
        assert_eq!(KokoroVoiceGroup::ChineseMale.label(), "中文男声");
        assert_eq!(KokoroVoiceGroup::EnglishFemale.label(), "英文女声");
    }
}
