/// Kokoro 内置说话人表（`kokoro-multi-lang-v1_1`，中英双语，103 说话人）。
///
/// sid → 官方说话人名的静态映射（来自
/// <https://k2-fsa.github.io/sherpa/onnx/tts/all/Chinese-English/kokoro-multi-lang-v1_1.html>，
/// 与 voices.bin 内部顺序一致）。sherpa-onnx Rust crate 未暴露按名字查 sid 的 API，
/// 因此以静态表为准；`OfflineTts::num_speakers()` 可做运行时兜底校验。
///
/// 命名规则：`af_*` 美式英文女声、`bf_*` 英式英文女声、`zf_*` 中文女声、`zm_*` 中文男声。
use crate::tts::voice::TtsVoice;

/// sid → 官方说话人名（v1_1 全量 103 项，fp32/int8 两包 voices.bin 相同）。
pub const KOKORO_SPEAKERS: &[(i32, &str)] = &[
    (0, "af_maple"),
    (1, "af_sol"),
    (2, "bf_vale"),
    (3, "zf_001"),
    (4, "zf_002"),
    (5, "zf_003"),
    (6, "zf_004"),
    (7, "zf_005"),
    (8, "zf_006"),
    (9, "zf_007"),
    (10, "zf_008"),
    (11, "zf_017"),
    (12, "zf_018"),
    (13, "zf_019"),
    (14, "zf_021"),
    (15, "zf_022"),
    (16, "zf_023"),
    (17, "zf_024"),
    (18, "zf_026"),
    (19, "zf_027"),
    (20, "zf_028"),
    (21, "zf_032"),
    (22, "zf_036"),
    (23, "zf_038"),
    (24, "zf_039"),
    (25, "zf_040"),
    (26, "zf_042"),
    (27, "zf_043"),
    (28, "zf_044"),
    (29, "zf_046"),
    (30, "zf_047"),
    (31, "zf_048"),
    (32, "zf_049"),
    (33, "zf_051"),
    (34, "zf_059"),
    (35, "zf_060"),
    (36, "zf_067"),
    (37, "zf_070"),
    (38, "zf_071"),
    (39, "zf_072"),
    (40, "zf_073"),
    (41, "zf_074"),
    (42, "zf_075"),
    (43, "zf_076"),
    (44, "zf_077"),
    (45, "zf_078"),
    (46, "zf_079"),
    (47, "zf_083"),
    (48, "zf_084"),
    (49, "zf_085"),
    (50, "zf_086"),
    (51, "zf_087"),
    (52, "zf_088"),
    (53, "zf_090"),
    (54, "zf_092"),
    (55, "zf_093"),
    (56, "zf_094"),
    (57, "zf_099"),
    (58, "zm_009"),
    (59, "zm_010"),
    (60, "zm_011"),
    (61, "zm_012"),
    (62, "zm_013"),
    (63, "zm_014"),
    (64, "zm_015"),
    (65, "zm_016"),
    (66, "zm_020"),
    (67, "zm_025"),
    (68, "zm_029"),
    (69, "zm_030"),
    (70, "zm_031"),
    (71, "zm_033"),
    (72, "zm_034"),
    (73, "zm_035"),
    (74, "zm_037"),
    (75, "zm_041"),
    (76, "zm_045"),
    (77, "zm_050"),
    (78, "zm_052"),
    (79, "zm_053"),
    (80, "zm_054"),
    (81, "zm_055"),
    (82, "zm_056"),
    (83, "zm_057"),
    (84, "zm_058"),
    (85, "zm_061"),
    (86, "zm_062"),
    (87, "zm_063"),
    (88, "zm_064"),
    (89, "zm_065"),
    (90, "zm_066"),
    (91, "zm_068"),
    (92, "zm_069"),
    (93, "zm_080"),
    (94, "zm_081"),
    (95, "zm_082"),
    (96, "zm_089"),
    (97, "zm_091"),
    (98, "zm_095"),
    (99, "zm_096"),
    (100, "zm_097"),
    (101, "zm_098"),
    (102, "zm_100"),
];

/// 说话人总数（sid 有效范围 0..=KOKORO_MAX_SID）。
pub const KOKORO_MAX_SID: i32 = 102;

/// 官方说话人名 → 中文友好显示名。
pub fn friendly_name(id: &str) -> String {
    if let Some(num) = id.strip_prefix("zf_") {
        format!("中文女声 {num}")
    } else if let Some(num) = id.strip_prefix("zm_") {
        format!("中文男声 {num}")
    } else if let Some(name) = id.strip_prefix("af_") {
        format!("英文女声 {name}")
    } else if let Some(name) = id.strip_prefix("bf_") {
        format!("英文女声（英式）{name}")
    } else {
        id.to_string()
    }
}

/// sid 是否在有效范围（防越界触发 sherpa C++ 崩溃）。
pub fn is_valid_sid(sid: i32) -> bool {
    (0..=KOKORO_MAX_SID).contains(&sid)
}

/// 按 sid 查说话人条目（复用 `TtsVoice`：`sid` 字段携带编号，参考音频字段为空）。
pub fn speaker_voice(sid: i32) -> Option<TtsVoice> {
    KOKORO_SPEAKERS
        .iter()
        .find(|(s, _)| *s == sid)
        .map(|(sid, id)| TtsVoice {
            id: (*id).to_string(),
            name: friendly_name(id),
            wav_path: std::path::PathBuf::new(),
            reference_text: String::new(),
            custom: false,
            sid: Some(*sid),
        })
}

/// 全部 103 个说话人（按 sid 升序）。
pub fn list_speakers() -> Vec<TtsVoice> {
    KOKORO_SPEAKERS
        .iter()
        .filter_map(|(sid, _)| speaker_voice(*sid))
        .collect()
}

/// 按官方说话人名查 sid（如 `zf_001` → 3）。
pub fn sid_by_name(name: &str) -> Option<i32> {
    KOKORO_SPEAKERS
        .iter()
        .find(|(_, id)| *id == name)
        .map(|(sid, _)| *sid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speaker_table_complete() {
        // 103 项、sid 0..=102 连续且唯一
        assert_eq!(KOKORO_SPEAKERS.len(), 103);
        for (i, (sid, _)) in KOKORO_SPEAKERS.iter().enumerate() {
            assert_eq!(*sid, i as i32, "sid 必须从 0 连续递增");
        }
    }

    #[test]
    fn test_speaker_table_naming_rules() {
        for (sid, id) in KOKORO_SPEAKERS {
            let valid = id.starts_with("zf_")
                || id.starts_with("zm_")
                || id.starts_with("af_")
                || id.starts_with("bf_");
            assert!(valid, "sid {sid} 名字 {id} 不符合命名规则");
        }
        // 英文说话人固定 3 个（0/1/2），中文女声 55 个（3..=57），中文男声 45 个（58..=102）
        assert_eq!(
            KOKORO_SPEAKERS
                .iter()
                .filter(|(_, id)| id.starts_with("zf_"))
                .count(),
            55
        );
        assert_eq!(
            KOKORO_SPEAKERS
                .iter()
                .filter(|(_, id)| id.starts_with("zm_"))
                .count(),
            45
        );
    }

    #[test]
    fn test_speaker_table_zf_zm_complementary() {
        // 官方 v1_1 的 zf/zm 编号互补（同一说话人群的女/男版本分开发布）
        let zf_nums: std::collections::BTreeSet<u32> = KOKORO_SPEAKERS
            .iter()
            .filter_map(|(_, id)| id.strip_prefix("zf_"))
            .map(|n| n.parse().unwrap())
            .collect();
        let zm_nums: std::collections::BTreeSet<u32> = KOKORO_SPEAKERS
            .iter()
            .filter_map(|(_, id)| id.strip_prefix("zm_"))
            .map(|n| n.parse().unwrap())
            .collect();
        for n in &zf_nums {
            assert!(!zm_nums.contains(n), "编号 {n} 不应同时出现在 zf 与 zm");
        }
        assert!(!zf_nums.is_empty() && !zm_nums.is_empty());
    }

    #[test]
    fn test_friendly_name() {
        assert_eq!(friendly_name("zf_001"), "中文女声 001");
        assert_eq!(friendly_name("zm_099"), "中文男声 099");
        assert_eq!(friendly_name("af_maple"), "英文女声 maple");
        assert_eq!(friendly_name("bf_vale"), "英文女声（英式）vale");
        assert_eq!(friendly_name("unknown"), "unknown");
    }

    #[test]
    fn test_speaker_voice_and_sid_by_name() {
        let v = speaker_voice(0).unwrap();
        assert_eq!(v.id, "af_maple");
        assert_eq!(v.name, "英文女声 maple");
        assert_eq!(v.sid, Some(0));
        assert!(v.wav_path.as_os_str().is_empty());
        assert!(!v.custom);

        let v = speaker_voice(57).unwrap();
        assert_eq!(v.id, "zf_099");
        assert_eq!(sid_by_name("zf_099"), Some(57));
        assert_eq!(sid_by_name("af_maple"), Some(0));

        assert!(speaker_voice(103).is_none());
        assert!(speaker_voice(-1).is_none());
        assert_eq!(sid_by_name("zf_999"), None);
    }

    #[test]
    fn test_list_speakers_order() {
        let speakers = list_speakers();
        assert_eq!(speakers.len(), 103);
        assert_eq!(speakers[0].sid, Some(0));
        assert_eq!(speakers[102].sid, Some(102));
        assert!(speakers.iter().all(|v| v.sid.is_some()));
    }

    #[test]
    fn test_is_valid_sid() {
        assert!(is_valid_sid(0));
        assert!(is_valid_sid(102));
        assert!(!is_valid_sid(103));
        assert!(!is_valid_sid(-1));
    }
}
