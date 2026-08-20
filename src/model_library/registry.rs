//! 模型库 Registry：编译期嵌入 `models/model_registry.json` 的目录解析。
//!
//! 一个 RegistryModel = 一个实际可加载的模型版本/变体（如 `qwen3-1.7b-q4-k-m`）。
//! 下载源（URL/sha256/size）不在此重复维护，而是通过 `download.manifest_role`
//! 引用 `models/manifest.json`（单一数据源）。

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::kws::model::{ModelAsset, asset_by_role};

/// 能力类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Kws,
    Asr,
    Llm,
    Tts,
}

impl ModelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelType::Kws => "kws",
            ModelType::Asr => "asr",
            ModelType::Llm => "llm",
            ModelType::Tts => "tts",
        }
    }

    pub fn from_str_value(s: &str) -> Option<Self> {
        match s {
            "kws" => Some(ModelType::Kws),
            "asr" => Some(ModelType::Asr),
            "llm" => Some(ModelType::Llm),
            "tts" => Some(ModelType::Tts),
            _ => None,
        }
    }
}

/// 顶层目录。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelRegistry {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    pub models: Vec<RegistryModel>,
}

/// 单个目录条目。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryModel {
    pub id: String,
    /// 目录基名（sherpa 模型目录名 / LLM 期望目录名）
    pub name: String,
    pub display_name: String,
    #[serde(rename = "model_type")]
    pub model_type: ModelType,
    pub runtime: String,
    pub format: String,
    pub description: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub parameter_count: Option<String>,
    #[serde(default)]
    pub quantization: Option<String>,
    /// LLM 条目：具体 GGUF 文件名
    #[serde(default)]
    pub file_name: Option<String>,
    pub version: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub homepage: Option<String>,
    /// 安装所需资产 role 列表（安装与完整性共用同一份定义）
    #[serde(default)]
    pub required_assets: Vec<String>,
    /// 可选增强资产 role 列表（如 ASR 的 punctuation，缺失不影响可用性）
    #[serde(default)]
    pub optional_assets: Vec<String>,
    /// `None` = 无内置下载源（需导入本地文件；当前 LLM 预设均已有 manifest 下载源）
    pub download: Option<RegistryDownload>,
}

impl RegistryModel {
    pub fn is_llm(&self) -> bool {
        self.model_type == ModelType::Llm
    }
}

/// 下载引用：只存 manifest role，真实 URL/hash/size 由 manifest 单源解析。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryDownload {
    pub manifest_role: String,
    #[serde(default)]
    pub extra_roles: Vec<String>,
    #[serde(default)]
    pub kind: String,
}

const REGISTRY_JSON: &str = include_str!("../../models/model_registry.json");

/// 解析一次并缓存。
fn registry() -> &'static ModelRegistry {
    static CACHE: OnceLock<ModelRegistry> = OnceLock::new();
    CACHE.get_or_init(|| serde_json::from_str(REGISTRY_JSON).expect("内嵌模型目录无效"))
}

/// 所有目录条目（保持 JSON 顺序，即推荐顺序）。
pub fn all_models() -> &'static [RegistryModel] {
    &registry().models
}

/// 按 id 查找目录条目。
pub fn model_by_id(id: &str) -> Option<&'static RegistryModel> {
    registry().models.iter().find(|m| m.id == id)
}

/// 按下载引用解析 manifest 资产。
pub fn asset_for(model: &RegistryModel) -> Option<&'static ModelAsset> {
    let role = model.download.as_ref()?.manifest_role.as_str();
    asset_by_role(role)
}

/// manifest role 对应的必需文件清单。
///
/// 安装（`install_asset_to` 的幂等/校验）与完整性判断使用**同一份**定义，
/// 避免出现「安装要求 A+B、完整性只查 A」的不一致。
pub fn required_files_for_role(role: &str) -> &'static [&'static str] {
    match role {
        "wake-word" => &crate::kws::model::KWS_REQUIRED_FILES,
        "wake-word-wenetspeech" => &crate::kws::model::KWS_WENETSPEECH_REQUIRED_FILES,
        // 所有 streaming zipformer ASR（含每个 ASR 的唯一 role）共用同一组 4 文件
        r if r == "asr" || r.starts_with("asr-") => &crate::asr::config::REQUIRED_FILES,
        "punctuation" => &crate::asr::config::PUNCT_REQUIRED_FILES,
        "tts" => &crate::tts::config::REQUIRED_FILES,
        "tts-vocoder" => &[crate::tts::config::DEFAULT_VOCODER],
        // LLM：必需文件由 `RegistryModel.file_name` 推导（见 install_managed_model），这里不维护静态表
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_parses() {
        let models = all_models();
        assert_eq!(
            models.len(),
            18,
            "应为 7 个首批（含 2 KWS）+ 5 个 ASR + 6 个补充 LLM"
        );
        assert!(
            models
                .iter()
                .all(|m| !m.id.is_empty() && !m.display_name.is_empty())
        );
        // id 唯一
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), models.len(), "Registry id 必须唯一");
        // LLM 条目：绑定具体 GGUF 文件名，且支持一键下载（raw 单文件）
        for m in models.iter().filter(|m| m.is_llm()) {
            assert!(m.file_name.is_some(), "LLM 条目必须绑定具体 GGUF 文件名");
            let d = m.download.as_ref().expect("LLM 应支持一键下载");
            assert_eq!(d.kind, "raw");
            assert_eq!(m.required_assets.len(), 1);
            assert_eq!(m.required_assets[0], d.manifest_role);
            assert!(m.format == "GGUF");
        }
    }

    #[test]
    fn test_registry_manifest_roles_exist() {
        // 所有 download.manifest_role / required_assets / optional_assets 都必须在 manifest 中存在
        for m in all_models() {
            if let Some(d) = &m.download {
                assert!(
                    asset_by_role(&d.manifest_role).is_some(),
                    "manifest_role '{}' 在 manifest 中不存在 (model {})",
                    d.manifest_role,
                    m.id
                );
                for extra in &d.extra_roles {
                    assert!(
                        asset_by_role(extra).is_some(),
                        "extra_roles '{}' 在 manifest 中不存在 (model {})",
                        extra,
                        m.id
                    );
                }
            }
            for role in m.required_assets.iter().chain(m.optional_assets.iter()) {
                assert!(
                    asset_by_role(role).is_some(),
                    "asset role '{}' 在 manifest 中不存在 (model {})",
                    role,
                    m.id
                );
            }
        }
    }

    #[test]
    fn test_model_by_id_and_order() {
        let m = model_by_id("qwen3-1.7b-q4-k-m").expect("按 id 查找");
        assert_eq!(m.model_type, ModelType::Llm);
        assert_eq!(m.file_name.as_deref(), Some("Qwen3-1.7B-Q4_K_M.gguf"));
        // 推荐顺序 = registry 原始顺序（首个是 KWS）
        assert_eq!(all_models()[0].model_type, ModelType::Kws);
    }

    #[test]
    fn test_required_files_for_role() {
        assert_eq!(required_files_for_role("asr").len(), 4);
        assert_eq!(required_files_for_role("punctuation").len(), 1);
        assert_eq!(required_files_for_role("tts").len(), 5); // 含 vocoder
        assert_eq!(required_files_for_role("tts-vocoder").len(), 1);
        assert_eq!(required_files_for_role("wake-word").len(), 5);
        // wenetspeech：epoch-12 三件套 + tokens + test_wavs/test_keywords.txt
        let ws = required_files_for_role("wake-word-wenetspeech");
        assert_eq!(ws.len(), 5);
        assert!(ws.contains(&"encoder-epoch-12-avg-2-chunk-16-left-64.onnx"));
        assert!(ws.contains(&"test_wavs/test_keywords.txt"));
        assert!(required_files_for_role("unknown").is_empty());
    }

    #[test]
    fn test_default_asset_stays_zh_en() {
        // manifest 中第一个 role=="wake-word" 资产必须保持 zh-en（default_asset 语义），
        // wenetspeech 用独立 role，不得排到 zh-en 之前。
        let d = crate::kws::model::default_asset();
        assert_eq!(d.role, "wake-word");
        assert_eq!(d.name, "sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20");
        let ws =
            crate::kws::model::asset_by_role("wake-word-wenetspeech").expect("wenetspeech 资产");
        assert_eq!(
            ws.name,
            "sherpa-onnx-kws-zipformer-wenetspeech-3.3M-2024-01-01"
        );
    }

    #[test]
    fn test_llm_preset_ids_for_download() {
        // LLM 配置页一键下载的预设（download_llm_model command 依赖），删除或改名会直接
        // 破坏下载按钮；同时钉住 name/file_name（幂等预检与条件写配置依赖该安装布局）。
        for (id, dir, file) in [
            ("qwen3-0.6b-q4-k-m", "Qwen3-0.6B", "Qwen3-0.6B-Q4_K_M.gguf"),
            (
                "qwen3-4b-instruct-2507-q4-k-m",
                "Qwen3-4B-Instruct-2507",
                "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
            ),
        ] {
            let m = model_by_id(id).unwrap_or_else(|| panic!("LLM 预设 {id} 必须存在"));
            assert!(m.is_llm() && m.download.is_some(), "{id} 应可一键下载");
            assert_eq!(m.name, dir);
            assert_eq!(m.file_name.as_deref(), Some(file));
        }
    }
}
