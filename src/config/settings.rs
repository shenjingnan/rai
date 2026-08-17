/// Settings - TOML 配置管理
///
/// 提供通用的配置读写功能，支持 ${env.VAR} 环境变量引用。
/// 配置文件存储在 `~/.zapmomo/settings.toml`。
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PROJECT_DIR: &str = ".zapmomo";
const SETTINGS_FILE: &str = "settings.toml";

/// 获取用户 home 目录（跨平台：macOS/Linux 用 $HOME，Windows 用 %USERPROFILE%）
pub fn get_home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
        .into()
}

/// 获取配置目录路径
pub fn get_settings_dir() -> PathBuf {
    get_home_dir().join(PROJECT_DIR)
}

/// 获取设置文件路径
pub fn get_settings_path() -> PathBuf {
    get_settings_dir().join(SETTINGS_FILE)
}

/// 获取模型目录路径：`~/.zapmomo/models`（模型统一安装到用户目录，不随仓库/安装包分发）
pub fn get_models_dir() -> PathBuf {
    get_settings_dir().join("models")
}

/// 获取 TTS 合成音频输出目录：`~/.zapmomo/tts`（供前端 asset 协议播放）。
pub fn get_tts_output_dir() -> PathBuf {
    get_settings_dir().join("tts")
}

/// 解析 ${env.VAR} 引用
///
/// - "${env.MY_VAR}" → 从环境变量 MY_VAR 读取
/// - "plain-value" → 原样返回
pub fn resolve_env_ref(value: &str) -> Result<String, String> {
    if let Some(captures) = value
        .strip_prefix("${env.")
        .and_then(|s| s.strip_suffix('}'))
    {
        let env_var = captures;
        if env_var.is_empty() {
            return Err("环境变量名称为空".to_string());
        }
        match std::env::var(env_var) {
            Ok(resolved) => Ok(resolved),
            Err(_) => Err(format!(
                "环境变量 {env_var} 未设置。请在 {SETTINGS_FILE} 中配置或设置环境变量 {env_var}。"
            )),
        }
    } else {
        Ok(value.to_string())
    }
}

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// 调试模式
    #[serde(default)]
    pub debug: bool,
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// 是否在 macOS Dock / Cmd+Tab 中隐藏应用图标（Accessory 模式），缺省 false 展示
    #[serde(default)]
    pub hide_dock_icon: bool,
    /// 自定义配置项（示例）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<std::collections::HashMap<String, String>>,
    /// 全局默认麦克风输入设备名（空 = 系统默认），KWS / ASR 共用；重启后免重选
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microphone: Option<String>,
    /// 唤醒词检测（KWS）配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kws: Option<KwsSettings>,
    /// 语音识别（ASR）配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asr: Option<AsrSettings>,
    /// 文本转语音（TTS）配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<TtsSettings>,
    /// Live2D 角色配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live2d: Option<Live2dSettings>,
    /// 本地 LLM 配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmSettings>,
    /// 模型库配置（用户通过「添加本地模型」注册的 external 模型等）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_library: Option<ModelLibrarySettings>,
}

/// 用户「添加本地模型」注册的模型（external）。
///
/// 只保存注册路径，**不复制/不管理用户文件**；移除时只删除本条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LocalModel {
    /// 稳定 id（`local-` + sha256(规范化绝对路径) 前 12 位）
    pub id: String,
    /// 目录/文件基名（展示用）
    pub name: String,
    /// 能力类型：kws | asr | llm | tts
    pub model_type: String,
    /// 绝对路径（LLM 必须是具体 .gguf 文件路径）
    pub path: String,
    /// 注册时间（RFC3339）
    pub added_at: String,
    /// 显式关联的 Registry 模型 id（从 Registry 卡片导入时携带；顶部添加本地模型为 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_id: Option<String>,
}

/// 模型库配置段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelLibrarySettings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_models: Vec<LocalModel>,
}

/// 唤醒词检测（KWS）配置。
///
/// 全部字段可缺省：未配置的项在解析时回退到 `kws::config` 的内置默认值，
/// 因此这里用 `Option` 以区分「未配置」与「配置了」。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct KwsSettings {
    /// 是否启用 KWS（打开开关即持久化；启动时自动监听的前提），缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// 会话级自定义唤醒词（原始字符串，多个用 / 分隔；持久化后启动自动监听也使用），缺省 None = 模型内置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_keywords: Option<String>,
    /// 模型目录（支持 ${env.VAR} 引用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<String>,
    /// encoder onnx 文件名（缺省用模型目录下 chunk-16 变体）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
    /// decoder onnx 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<String>,
    /// joiner onnx 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joiner: Option<String>,
    /// tokens.txt 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    /// 关键词文件路径（缺省 = <model_dir>/test_wavs/keywords.txt）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords_file: Option<String>,
    /// 推理后端，缺省 "cpu"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// 推理线程数，缺省 2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_threads: Option<i32>,
    /// 每次喂给模型的采样数（@16k），缺省 3200（0.2s）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    /// 模型输入采样率，缺省 16000
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<i32>,
    /// 关键词 boosting 分数，缺省 1.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords_score: Option<f32>,
    /// 触发阈值，缺省 0.25（越大越不容易误触发）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords_threshold: Option<f32>,
    /// 调试输出，缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,
}

/// 语音识别（ASR）配置。
///
/// 全部字段可缺省：未配置的项在解析时回退到 `asr::config` 的内置默认值，
/// 因此这里用 `Option` 以区分「未配置」与「配置了」。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AsrSettings {
    /// 模型目录（支持 ${env.VAR} 引用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<String>,
    /// encoder onnx 文件名（缺省用 int8 变体）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
    /// decoder onnx 文件名（缺省用 fp32 变体，官方 int8 配方）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<String>,
    /// joiner onnx 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joiner: Option<String>,
    /// tokens.txt 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    /// 推理后端，缺省 "cpu"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// 推理线程数，缺省 2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_threads: Option<i32>,
    /// 每次喂给模型的采样数（@16k），缺省 3200（0.2s）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    /// 模型输入采样率，缺省 16000
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<i32>,
    /// 解码方式：greedy_search | modified_beam_search，缺省 greedy_search
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoding_method: Option<String>,
    /// 端点检测（静音自动断句），缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_endpoint: Option<bool>,
    /// 规则 1 最小尾随静音（秒），缺省 2.4
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule1_min_trailing_silence: Option<f32>,
    /// 规则 2 最小尾随静音（秒），缺省 1.2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule2_min_trailing_silence: Option<f32>,
    /// 规则 3 最小句长（秒），缺省 20.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule3_min_utterance_length: Option<f32>,
    /// 空白符惩罚，缺省 0.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blank_penalty: Option<f32>,
    /// 热词（空格分隔，中文直接写），缺省无
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotwords: Option<String>,
    /// 是否对最终结果自动加标点，缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_punctuation: Option<bool>,
    /// 标点模型 onnx 路径（相对路径锚定标点模型目录）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub punctuation_model: Option<String>,
    /// 调试输出，缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,
}

/// 文本转语音（TTS）配置。
///
/// 全部字段可缺省：未配置的项在解析时回退到 `tts::config` 的内置默认值，
/// 因此这里用 `Option` 以区分「未配置」与「配置了」。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TtsSettings {
    /// 是否启用语音合成，缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// 模型目录（支持 ${env.VAR} 引用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<String>,
    /// encoder onnx 文件名（缺省 int8 变体）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
    /// decoder onnx 文件名（缺省 int8 变体）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<String>,
    /// 声码器 vocoder onnx 文件名（缺省 vocos_24khz.onnx）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vocoder: Option<String>,
    /// tokens.txt 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    /// lexicon.txt 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexicon: Option<String>,
    /// espeak-ng 数据目录名（缺省 espeak-ng-data）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    /// 参考音频 wav 路径（相对模型目录；缺省 test_wavs/leijun-1.wav）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_wav: Option<String>,
    /// 参考音频的逐字转写文本
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_text: Option<String>,
    /// 扩散解码步数（质量/速度权衡），缺省 4
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_steps: Option<i32>,
    /// 语速，缺省 1.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    /// 推理后端，缺省 "cpu"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// 推理线程数，缺省 2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_threads: Option<i32>,
    /// 调试输出，缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,
}

/// 角色窗口位置（逻辑像素）。
///
/// `None` 表示未记录 → 首次启动时定位到屏幕右下角；记录后用于恢复手动拖动的位置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CompanionWindowPosition {
    /// 窗口左上角 x 坐标（逻辑像素）。
    pub x: i32,
    /// 窗口左上角 y 坐标（逻辑像素）。
    pub y: i32,
}

/// Live2D 角色配置。
///
/// 字段可缺省：未配置时回退到 `live2d::config` 的默认目录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Live2dSettings {
    /// 模型目录（支持 ${env.VAR} 引用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<String>,
    /// 角色窗口位置（逻辑像素；缺省表示未记录）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_position: Option<CompanionWindowPosition>,
    /// 角色窗口缩放比例（1.0 = 100%；缺省视为 1.0）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_scale: Option<f64>,
}

/// 本地 LLM 配置。
///
/// 全部字段可缺省：未配置的项在解析时回退到 `llm::config` 的内置默认值。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LlmSettings {
    /// 是否启用 Local LLM，缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// provider 标识，缺省 "local"（未来 ollama/openai/...）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// GGUF 模型文件路径（支持 ${env.VAR}，相对路径锚定 ~/.zapmomo/models）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<String>,
    /// 角色 system prompt，缺省用内置默认
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// 上下文窗口大小（token），缺省 8192
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_size: Option<usize>,
    /// 单次 decode batch 大小，缺省 512
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,
    /// 最多生成 token 数，缺省 512
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    /// 温度，缺省 0.7
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// top_p 采样，缺省 0.8
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// top_k 采样，缺省 20
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// min_p 采样，缺省 0.05
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,
    /// 重复惩罚，缺省 1.05
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    /// 随机种子，缺省 0（随机）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    /// CPU 线程数，缺省 物理核数 - 2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<i32>,
    /// 卸载到 GPU 的层数，缺省 -1（全部，Metal）；0 表示纯 CPU
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_layers: Option<i32>,
    /// 是否开启 Qwen3 思考模式（输出 <think> 块），缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_thinking: Option<bool>,
    /// 是否在应用启动时自动加载模型，缺省 false（懒加载）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_load: Option<bool>,
    /// HTTP provider 的 base URL（如 https://api.openai.com/v1 或 http://127.0.0.1:8080/v1）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// HTTP provider 的 API key（本地 server 可留空）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// HTTP provider 的模型名（如 qwen3-4b / gpt-4o-mini）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            debug: false,
            log_level: default_log_level(),
            hide_dock_icon: false,
            custom: None,
            microphone: None,
            kws: None,
            asr: None,
            tts: None,
            live2d: None,
            llm: None,
            model_library: None,
        }
    }
}

/// 加载 ~/.zapmomo/settings.toml
///
/// 文件不存在时返回 None，不报错。
pub fn load_settings() -> Result<Option<AppConfig>, String> {
    let file_path = get_settings_path();

    let content = match std::fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };

    let config: AppConfig = toml::from_str(&content).map_err(|e| format!("TOML 格式错误: {e}"))?;

    Ok(Some(config))
}

/// 保存配置到 `~/.zapmomo/settings.toml`（自动创建父目录）。
///
/// 采用「临时文件 + 替换」的安全写：先把完整内容写入带 pid 后缀的临时文件，
/// 再 rename 到正式路径。POSIX 上 rename 同文件系统是原子的（直接覆盖）；Windows
/// 上 rename 无法覆盖已存在目标，先移除旧文件再 rename（存在短暂窗口）。若替换失败
/// 会保留临时文件便于恢复，并返回明确错误——**不做严格 atomic replace 的承诺**。
pub fn save_settings(config: &AppConfig) -> Result<(), String> {
    let file_path = get_settings_path();
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {e}"))?;
    let tmp = file_path.with_file_name(format!("settings.toml.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &content).map_err(|e| format!("写入临时配置失败: {e}"))?;
    match std::fs::rename(&tmp, &file_path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows：目标存在时 rename 可能失败，先移除再重试；失败则保留 tmp 便于恢复。
            if file_path.exists() {
                std::fs::remove_file(&file_path).map_err(|e| format!("移除旧配置失败: {e}"))?;
            }
            std::fs::rename(&tmp, &file_path).map_err(|e| format!("替换配置失败: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::run_with_temp_home;

    fn write_toml_settings(home: &std::path::Path, content: &str) {
        let settings_dir = home.join(PROJECT_DIR);
        std::fs::create_dir_all(&settings_dir).unwrap();
        std::fs::write(settings_dir.join(SETTINGS_FILE), content).unwrap();
    }

    #[test]
    fn test_get_settings_path() {
        run_with_temp_home(|home| {
            let path = get_settings_path();
            assert_eq!(path, home.join(".zapmomo/settings.toml"));
        });
    }

    #[test]
    fn test_get_settings_dir() {
        run_with_temp_home(|home| {
            let dir = get_settings_dir();
            assert_eq!(dir, home.join(".zapmomo"));
        });
    }

    #[test]
    fn test_resolve_env_ref_plain_value() {
        assert_eq!(resolve_env_ref("plain-value").unwrap(), "plain-value");
        assert_eq!(
            resolve_env_ref("https://example.com").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn test_resolve_env_ref_from_env() {
        unsafe {
            std::env::set_var("TEST_MY_VAR", "test-resolved-value");
        }
        assert_eq!(
            resolve_env_ref("${env.TEST_MY_VAR}").unwrap(),
            "test-resolved-value"
        );
        unsafe {
            std::env::remove_var("TEST_MY_VAR");
        }
    }

    #[test]
    fn test_resolve_env_ref_missing_var() {
        let result = resolve_env_ref("${env.NONEXISTENT_VAR_XYZ}");
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("NONEXISTENT_VAR_XYZ"));
    }

    #[test]
    fn test_resolve_env_ref_empty() {
        assert_eq!(resolve_env_ref("").unwrap(), "");
    }

    #[test]
    fn test_resolve_env_ref_empty_env_var_name() {
        let result = resolve_env_ref("${env.}");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_settings_file_not_found() {
        run_with_temp_home(|_| {
            let result = load_settings().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn test_load_settings_invalid_toml() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "{invalid}");
            let result = load_settings();
            assert!(result.is_err());
            assert!(result.err().unwrap().contains("TOML 格式错误"));
        });
    }

    #[test]
    fn test_load_settings_empty() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "");
            let result = load_settings().unwrap().unwrap();
            assert!(!result.debug);
            assert_eq!(result.log_level, "info");
            assert!(result.custom.is_none());
        });
    }

    #[test]
    fn test_load_settings_full() {
        run_with_temp_home(|home| {
            write_toml_settings(
                home,
                "debug = true\nlog_level = \"debug\"\n\n[custom]\nkey1 = \"value1\"\n",
            );
            let result = load_settings().unwrap().unwrap();
            assert!(result.debug);
            assert_eq!(result.log_level, "debug");
            assert_eq!(result.custom.unwrap().get("key1").unwrap(), "value1");
        });
    }

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(!config.debug);
        assert_eq!(config.log_level, "info");
        assert!(config.custom.is_none());
        assert!(config.microphone.is_none());
    }

    #[test]
    fn test_app_config_serde_roundtrip() {
        let config = AppConfig {
            debug: true,
            log_level: "warn".to_string(),
            hide_dock_icon: true,
            custom: Some(std::collections::HashMap::new()),
            microphone: Some("内置麦克风".to_string()),
            kws: None,
            asr: None,
            tts: None,
            live2d: None,
            llm: None,
            model_library: None,
        };
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, deserialized);
        // 缺省字段应被反序列化为 false；microphone 应被序列化
        assert!(toml_str.contains("hide_dock_icon"));
        assert!(toml_str.contains("microphone"));
    }

    #[test]
    fn test_load_settings_without_hide_dock_icon_defaults_false() {
        // 旧配置文件没有 hide_dock_icon 字段时，应回退为 false（默认展示图标）。
        run_with_temp_home(|home| {
            write_toml_settings(home, "debug = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(!result.hide_dock_icon);
        });
    }

    #[test]
    fn test_load_settings_with_hide_dock_icon() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "hide_dock_icon = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(result.hide_dock_icon);
        });
    }

    #[test]
    fn test_load_settings_with_microphone() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "microphone = \"内置麦克风\"\n");
            let result = load_settings().unwrap().unwrap();
            assert_eq!(result.microphone.as_deref(), Some("内置麦克风"));
        });
    }

    #[test]
    fn test_load_settings_with_asr_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[asr]\nnum_threads = 4\nenable_endpoint = false\n");
            let result = load_settings().unwrap().unwrap();
            let asr = result.asr.unwrap();
            assert_eq!(asr.num_threads, Some(4));
            assert_eq!(asr.enable_endpoint, Some(false));
            // 未配置的字段保持 None
            assert_eq!(asr.model_dir, None);
            assert_eq!(asr.decoding_method, None);
        });
    }

    #[test]
    fn test_load_settings_without_asr_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "debug = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(result.asr.is_none());
        });
    }

    #[test]
    fn test_load_settings_with_kws_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[kws]\nnum_threads = 4\nchunk_size = 1600\n");
            let result = load_settings().unwrap().unwrap();
            let kws = result.kws.unwrap();
            assert_eq!(kws.num_threads, Some(4));
            assert_eq!(kws.chunk_size, Some(1600));
            // 未配置的字段保持 None
            assert_eq!(kws.model_dir, None);
            assert_eq!(kws.keywords_threshold, None);
        });
    }

    #[test]
    fn test_load_settings_without_kws_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "debug = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(result.kws.is_none());
        });
    }

    #[test]
    fn test_load_settings_with_tts_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[tts]\nnum_threads = 4\nspeed = 1.2\n");
            let result = load_settings().unwrap().unwrap();
            let tts = result.tts.unwrap();
            assert_eq!(tts.num_threads, Some(4));
            assert_eq!(tts.speed, Some(1.2));
            // 未配置的字段保持 None
            assert_eq!(tts.model_dir, None);
            assert_eq!(tts.num_steps, None);
        });
    }

    #[test]
    fn test_load_settings_without_tts_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "debug = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(result.tts.is_none());
        });
    }

    #[test]
    fn test_load_settings_with_tts_enabled_false() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[tts]\nenabled = false\n");
            let result = load_settings().unwrap().unwrap();
            let tts = result.tts.unwrap();
            assert_eq!(tts.enabled, Some(false));
        });
    }

    #[test]
    fn test_tts_settings_serde_roundtrip() {
        let tts = TtsSettings {
            enabled: Some(false),
            model_dir: Some("${env.TTS_MODEL_DIR}".to_string()),
            encoder: Some("encoder.int8.onnx".to_string()),
            decoder: None,
            vocoder: Some("vocos_24khz.onnx".to_string()),
            tokens: None,
            lexicon: None,
            data_dir: None,
            reference_wav: Some("test_wavs/leijun-1.wav".to_string()),
            reference_text: None,
            num_steps: Some(4),
            speed: Some(1.0),
            provider: Some("cpu".to_string()),
            num_threads: Some(2),
            debug: Some(false),
        };
        let toml_str = toml::to_string(&tts).unwrap();
        let deserialized: TtsSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(tts, deserialized);
        // 未配置字段应被 skip_serializing_if 忽略
        assert!(!toml_str.contains("decoder"));
    }

    #[test]
    fn test_get_tts_output_dir() {
        run_with_temp_home(|home| {
            assert_eq!(get_tts_output_dir(), home.join(".zapmomo/tts"));
        });
    }

    #[test]
    fn test_kws_settings_serde_roundtrip() {
        let kws = KwsSettings {
            enabled: Some(false),
            custom_keywords: Some("你好小智".to_string()),
            model_dir: Some("${env.KWS_MODEL_DIR}".to_string()),
            encoder: Some("encoder.onnx".to_string()),
            decoder: None,
            joiner: None,
            tokens: None,
            keywords_file: Some("kw.txt".to_string()),
            provider: Some("cpu".to_string()),
            num_threads: Some(4),
            chunk_size: Some(1600),
            sample_rate: Some(16000),
            keywords_score: Some(1.0),
            keywords_threshold: Some(0.3),
            debug: Some(false),
        };
        let toml_str = toml::to_string(&kws).unwrap();
        let deserialized: KwsSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(kws, deserialized);
        // 未配置字段应被 skip_serializing_if 忽略
        assert!(!toml_str.contains("decoder"));
    }

    #[test]
    fn test_kws_settings_env_ref_resolution() {
        unsafe {
            std::env::set_var("KWS_MODEL_DIR", "/tmp/kws-model");
        }
        let kws = KwsSettings {
            model_dir: Some("${env.KWS_MODEL_DIR}".to_string()),
            ..KwsSettings::default()
        };
        assert_eq!(
            resolve_env_ref(kws.model_dir.as_ref().unwrap()).unwrap(),
            "/tmp/kws-model"
        );
        unsafe {
            std::env::remove_var("KWS_MODEL_DIR");
        }
    }

    #[test]
    fn test_live2d_settings_serde_roundtrip() {
        let live2d = Live2dSettings {
            model_dir: Some("/tmp/some-model".to_string()),
            window_position: Some(CompanionWindowPosition { x: 120, y: 800 }),
            window_scale: Some(1.5),
        };
        let toml_str = toml::to_string(&live2d).unwrap();
        let deserialized: Live2dSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(live2d, deserialized);
        // 未记录位置/比例时字段应被 skip_serializing_if 忽略
        let none_pos = Live2dSettings {
            model_dir: Some("/tmp/some-model".to_string()),
            window_position: None,
            window_scale: None,
        };
        let none_toml = toml::to_string(&none_pos).unwrap();
        assert!(!none_toml.contains("window_position"));
        assert!(!none_toml.contains("window_scale"));
    }

    #[test]
    fn test_load_settings_with_live2d_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[live2d]\nmodel_dir = \"/tmp/model-dir\"\n");
            let result = load_settings().unwrap().unwrap();
            let live2d = result.live2d.unwrap();
            assert_eq!(live2d.model_dir.as_deref(), Some("/tmp/model-dir"));
        });
    }

    #[test]
    fn test_save_settings_roundtrip() {
        run_with_temp_home(|home| {
            let config = AppConfig {
                debug: true,
                log_level: "debug".to_string(),
                hide_dock_icon: false,
                custom: None,
                microphone: None,
                kws: None,
                asr: None,
                tts: None,
                live2d: Some(Live2dSettings {
                    model_dir: Some("/tmp/model-dir".to_string()),
                    ..Default::default()
                }),
                llm: None,
                model_library: None,
            };
            save_settings(&config).unwrap();
            let loaded = load_settings().unwrap().unwrap();
            assert_eq!(loaded, config);
            // 文件确实写到了 HOME 下
            assert!(home.join(".zapmomo/settings.toml").is_file());
        });
    }

    #[test]
    fn test_model_library_settings_roundtrip() {
        run_with_temp_home(|home| {
            let config = AppConfig {
                model_library: Some(ModelLibrarySettings {
                    local_models: vec![LocalModel {
                        id: "local-abcdef123456".to_string(),
                        name: "MyModel.gguf".to_string(),
                        model_type: "llm".to_string(),
                        path: "/tmp/models/MyModel.gguf".to_string(),
                        added_at: "2026-08-17T00:00:00Z".to_string(),
                        registry_id: Some("qwen3-1.7b-q4-k-m".to_string()),
                    }],
                }),
                ..Default::default()
            };
            save_settings(&config).unwrap();
            let loaded = load_settings().unwrap().unwrap();
            assert_eq!(loaded, config);
            // external 注册信息存在 settings 中，不写用户模型目录
            assert!(home.join(".zapmomo/settings.toml").is_file());
            // 缺省 local_models 不序列化
            let empty = AppConfig {
                model_library: Some(ModelLibrarySettings::default()),
                ..Default::default()
            };
            let toml_str = toml::to_string(&empty).unwrap();
            assert!(!toml_str.contains("local_models"));
        });
    }

    #[test]
    fn test_save_settings_safe_replace_and_tmp_cleanup() {
        run_with_temp_home(|home| {
            let config = AppConfig {
                log_level: "debug".to_string(),
                ..Default::default()
            };
            save_settings(&config).unwrap();
            // 正式文件存在
            assert!(home.join(".zapmomo/settings.toml").is_file());
            // 临时文件被清理（rename 成功）
            let tmp = home.join(format!(".zapmomo/settings.toml.tmp.{}", std::process::id()));
            assert!(!tmp.exists());
            // 覆盖保存仍成功且内容完整
            let config2 = AppConfig {
                log_level: "warn".to_string(),
                ..Default::default()
            };
            save_settings(&config2).unwrap();
            let loaded = load_settings().unwrap().unwrap();
            assert_eq!(loaded.log_level, "warn");
        });
    }
}
