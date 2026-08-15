use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "zapmomo",
    version = VERSION,
    about = "An open-source, real-time desktop AI companion with voice, memory, and a customizable virtual character",
    subcommand_required = true,
    arg_required_else_help = true,
    disable_help_subcommand = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
#[non_exhaustive]
pub enum Commands {
    /// 显示配置信息
    Config,
    /// 向用户问好（演示命令参数用法）
    Greet {
        /// 你的名字
        #[arg(short, long)]
        name: String,
        /// 重复次数
        #[arg(short, long, default_value = "1")]
        count: u32,
    },
    /// 生成 Shell 补全脚本
    #[command(hide = true)]
    Completion {
        /// Shell 类型：bash、zsh、fish、powershell、elvish
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// 关键词唤醒词检测（KWS）
    Kws {
        #[command(subcommand)]
        cmd: KwsCmd,
    },
    /// 语音识别（ASR）
    Asr {
        #[command(subcommand)]
        cmd: AsrCmd,
    },
    /// 文本转语音（TTS）
    Tts {
        #[command(subcommand)]
        cmd: TtsCmd,
    },
    /// 本地大语言模型（LLM）
    Llm {
        #[command(subcommand)]
        cmd: LlmCmd,
    },
}

/// KWS 子命令
#[derive(Subcommand)]
pub enum KwsCmd {
    /// 实时监听麦克风，检测唤醒词
    Run {
        /// 模型目录（覆盖 settings.toml 的 kws.model_dir）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 运行时附加关键词（tokenized 格式，多个用 / 分隔）
        #[arg(long)]
        keywords: Option<String>,
        /// 指定输入设备名（包含匹配），默认系统默认麦克风
        #[arg(long)]
        device: Option<String>,
        /// 监听时长（秒），默认无限
        #[arg(long)]
        duration: Option<u64>,
    },
    /// 离线检测 wav 文件中的关键词（不需要麦克风）
    Test {
        /// wav 路径；默认 <model_dir>/test_wavs/zh_3.wav
        #[arg(long)]
        wav: Option<PathBuf>,
        #[arg(long)]
        model_dir: Option<PathBuf>,
        #[arg(long)]
        keywords: Option<String>,
    },
    /// 列出可用的麦克风输入设备
    Devices,
    /// 下载并安装唤醒词模型（默认安装到 ~/.zapmomo/models/<模型名>）
    InstallModel {
        /// 安装目标模型目录（默认 ~/.zapmomo/models/<模型名>）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 已安装也强制重新下载
        #[arg(long)]
        force: bool,
    },
}

/// ASR 子命令
#[derive(Subcommand)]
pub enum AsrCmd {
    /// 实时监听麦克风并转写
    Run {
        /// 模型目录（覆盖 settings.toml 的 asr.model_dir）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 指定输入设备名（包含匹配），默认系统默认麦克风
        #[arg(long)]
        device: Option<String>,
        /// 监听时长（秒），默认无限
        #[arg(long)]
        duration: Option<u64>,
        /// 热词（空格分隔，中文直接写），提升专有名词识别
        #[arg(long)]
        hotwords: Option<String>,
    },
    /// 离线转写 wav 文件（不需要麦克风）
    Test {
        /// wav 路径；默认 <model_dir>/test_wavs/0.wav
        #[arg(long)]
        wav: Option<PathBuf>,
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 热词（空格分隔，中文直接写），提升专有名词识别
        #[arg(long)]
        hotwords: Option<String>,
    },
    /// 列出可用的麦克风输入设备
    Devices,
    /// 下载并安装 ASR 模型（默认安装到 ~/.zapmomo/models/<模型名>）
    InstallModel {
        /// 安装目标模型目录（默认 ~/.zapmomo/models/<模型名>）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 已安装也强制重新下载
        #[arg(long)]
        force: bool,
    },
}

/// TTS 子命令
#[derive(Subcommand)]
pub enum TtsCmd {
    /// 把文本合成为 wav 文件
    Run {
        /// 要合成的文本
        #[arg(short, long)]
        text: String,
        /// 模型目录（覆盖 settings.toml 的 tts.model_dir）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 语速，缺省 1.0
        #[arg(long)]
        speed: Option<f32>,
        /// 输出 wav 路径；缺省 ~/.zapmomo/tts/<时间戳>.wav
        #[arg(long)]
        output: Option<PathBuf>,
        /// 内置音色 id（如 leijun-1 / news-female / news-female-2）
        #[arg(long)]
        voice: Option<String>,
        /// 自定义参考音频 wav（配合 --reference-text 使用）
        #[arg(long)]
        reference_wav: Option<PathBuf>,
        /// 自定义参考音频的逐字转写文本
        #[arg(long)]
        reference_text: Option<String>,
    },
    /// 列出可用的内置音色
    Voices {
        /// 模型目录（覆盖 settings.toml 的 tts.model_dir）
        #[arg(long)]
        model_dir: Option<PathBuf>,
    },
    /// 下载并安装 TTS 模型（主包 + 声码器，默认 ~/.zapmomo/models/<模型名>）
    InstallModel {
        /// 安装目标模型目录（默认 ~/.zapmomo/models/<模型名>）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 已安装也强制重新下载
        #[arg(long)]
        force: bool,
    },
}

/// LLM 子命令
#[derive(Subcommand)]
pub enum LlmCmd {
    /// 加载模型并打印信息（验证模型可用）
    Load {
        /// GGUF 模型文件路径（覆盖 settings.toml 的 llm.model_path）
        #[arg(long)]
        model_path: Option<PathBuf>,
    },
    /// 单轮对话（加载 + 流式生成）
    Chat {
        /// 用户输入文本
        #[arg(short, long)]
        text: String,
        /// GGUF 模型文件路径（覆盖 settings.toml 的 llm.model_path）
        #[arg(long)]
        model_path: Option<PathBuf>,
    },
}

/// config 命令
fn cmd_config() -> Result<String, String> {
    let config = serde_json::json!({
        "version": VERSION,
        "debug": false,
        "logLevel": "info",
    });
    Ok(serde_json::to_string_pretty(&config).unwrap_or_default())
}

/// greet 命令
fn cmd_greet(name: &str, count: u32) -> Result<(), String> {
    for _ in 0..count {
        println!("你好, {name}！欢迎使用 ZapMomo。");
    }
    Ok(())
}

/// completion 命令
fn cmd_completion<W: std::io::Write>(shell: clap_complete::Shell, writer: &mut W) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "zapmomo", writer);
}

/// CLI 入口
pub async fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Commands::Config) => {
            let output = cmd_config()?;
            println!("{output}");
            Ok(())
        }
        Some(Commands::Greet { name, count }) => cmd_greet(&name, count),
        Some(Commands::Completion { shell }) => {
            cmd_completion(shell, &mut std::io::stdout());
            Ok(())
        }
        Some(Commands::Kws { cmd }) => cmd_kws(cmd).await,
        Some(Commands::Asr { cmd }) => cmd_asr(cmd).await,
        Some(Commands::Tts { cmd }) => cmd_tts(cmd).await,
        Some(Commands::Llm { cmd }) => cmd_llm(cmd).await,
        None => unreachable!(),
    }
}

/// KWS 命令入口
async fn cmd_kws(cmd: KwsCmd) -> Result<(), String> {
    match cmd {
        KwsCmd::Run {
            model_dir,
            keywords,
            device,
            duration,
        } => {
            let cfg = kws_config(model_dir.as_ref())?;
            crate::kws::run_realtime(&cfg, device.as_deref(), duration, keywords.as_deref())
        }
        KwsCmd::Test {
            wav,
            model_dir,
            keywords,
        } => {
            let cfg = kws_config(model_dir.as_ref())?;
            let wav_path = wav.unwrap_or_else(|| cfg.model_dir.join("test_wavs/zh_3.wav"));
            crate::kws::run_offline(&cfg, &wav_path, keywords.as_deref())
        }
        KwsCmd::Devices => {
            let devices = crate::audio::list_input_devices();
            if devices.is_empty() {
                println!("未找到任何输入设备。");
            } else {
                println!("可用输入设备:");
                for name in devices {
                    println!("  {name}");
                }
            }
            Ok(())
        }
        KwsCmd::InstallModel { model_dir, force } => {
            use crate::kws::model::{
                DownloadProgress, DownloadStage, install_model_to, user_model_dir,
            };
            let dest = model_dir.unwrap_or_else(user_model_dir);
            let mut progress = |p: DownloadProgress| {
                let stage = match p.stage {
                    DownloadStage::Downloading => "下载",
                    DownloadStage::Verifying => "校验",
                    DownloadStage::Extracting => "解压",
                    DownloadStage::Done => "完成",
                };
                println!("[{stage}] {}", p.message);
            };
            install_model_to(&dest, force, &mut progress).map_err(|e| e.to_string())?;
            println!("模型已就绪: {}", dest.display());
            Ok(())
        }
    }
}

/// 读取 settings 并解析 KWS 配置
fn kws_config(
    cli_model_dir: Option<&PathBuf>,
) -> Result<crate::kws::config::ResolvedKwsConfig, String> {
    let settings = crate::config::settings::load_settings()?;
    let kws_settings = settings.as_ref().and_then(|s| s.kws.clone());
    crate::kws::config::resolve(kws_settings.as_ref(), cli_model_dir.map(|p| p.as_path()))
}

/// ASR 命令入口
async fn cmd_asr(cmd: AsrCmd) -> Result<(), String> {
    match cmd {
        AsrCmd::Run {
            model_dir,
            device,
            duration,
            hotwords,
        } => {
            let mut cfg = asr_config(model_dir.as_ref())?;
            if hotwords.is_some() {
                cfg.hotwords = hotwords;
            }
            crate::asr::run_realtime(&cfg, device.as_deref(), duration)
        }
        AsrCmd::Test {
            wav,
            model_dir,
            hotwords,
        } => {
            let mut cfg = asr_config(model_dir.as_ref())?;
            if hotwords.is_some() {
                cfg.hotwords = hotwords;
            }
            let wav_path = wav.unwrap_or_else(|| cfg.model_dir.join("test_wavs/0.wav"));
            crate::asr::run_offline(&cfg, &wav_path)
        }
        AsrCmd::Devices => {
            let devices = crate::audio::list_input_devices();
            if devices.is_empty() {
                println!("未找到任何输入设备。");
            } else {
                println!("可用输入设备:");
                for name in devices {
                    println!("  {name}");
                }
            }
            Ok(())
        }
        AsrCmd::InstallModel { model_dir, force } => {
            use crate::asr::{
                DownloadProgress, DownloadStage, install_model_to, install_punctuation_model_to,
                punctuation_user_model_dir, user_model_dir,
            };
            let dest = model_dir.unwrap_or_else(user_model_dir);
            let mut progress = |p: DownloadProgress| {
                let stage = match p.stage {
                    DownloadStage::Downloading => "下载",
                    DownloadStage::Verifying => "校验",
                    DownloadStage::Extracting => "解压",
                    DownloadStage::Done => "完成",
                };
                println!("[{stage}] {}", p.message);
            };
            install_model_to(&dest, force, &mut progress).map_err(|e| e.to_string())?;
            println!("ASR 模型已就绪: {}", dest.display());

            // 顺带安装标点模型（自动开启）；失败仅警告，不阻断 ASR。
            let punct_dest = punctuation_user_model_dir();
            match install_punctuation_model_to(&punct_dest, force, &mut progress) {
                Ok(()) => println!("标点模型已就绪: {}", punct_dest.display()),
                Err(e) => eprintln!("警告：标点模型安装失败（ASR 仍可用，仅无标点）: {e}"),
            }
            Ok(())
        }
    }
}

/// 读取 settings 并解析 ASR 配置
fn asr_config(
    cli_model_dir: Option<&PathBuf>,
) -> Result<crate::asr::config::ResolvedAsrConfig, String> {
    let settings = crate::config::settings::load_settings()?;
    let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
    crate::asr::config::resolve(asr_settings.as_ref(), cli_model_dir.map(|p| p.as_path()))
}

/// TTS 命令入口
async fn cmd_tts(cmd: TtsCmd) -> Result<(), String> {
    match cmd {
        TtsCmd::Run {
            text,
            model_dir,
            speed,
            output,
            voice,
            reference_wav,
            reference_text,
        } => {
            let cfg = tts_config(model_dir.as_ref())?;
            let engine = crate::tts::TtsEngine::new(cfg.clone())?;
            let speed = speed.unwrap_or(1.0);
            let (ref_wav, ref_text) = crate::tts::voice::resolve_reference(
                &cfg,
                voice.as_deref(),
                reference_wav.as_deref(),
                reference_text.as_deref(),
            )?;
            let out_path = output.unwrap_or_else(crate::tts::default_output_path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败: {e}"))?;
            }
            engine.synthesize_to_wav(&text, speed, &ref_wav, &ref_text, &out_path)?;
            println!("已合成: {}", out_path.display());
            Ok(())
        }
        TtsCmd::Voices { model_dir } => {
            let cfg = tts_config(model_dir.as_ref())?;
            let voices = crate::tts::voice::list_builtin_voices(&cfg.model_dir);
            if voices.is_empty() {
                println!("未找到内置音色（请先运行 `zapmomo tts install-model` 下载模型）。");
            } else {
                println!("可用内置音色:");
                for v in voices {
                    println!("  {}  {}", v.id, v.name);
                }
            }
            Ok(())
        }
        TtsCmd::InstallModel { model_dir, force } => {
            use crate::tts::{
                DownloadProgress, DownloadStage, install_model_to, install_vocoder_to,
                user_model_dir,
            };
            let dest = model_dir.unwrap_or_else(user_model_dir);
            let mut progress = |p: DownloadProgress| {
                let stage = match p.stage {
                    DownloadStage::Downloading => "下载",
                    DownloadStage::Verifying => "校验",
                    DownloadStage::Extracting => "解压",
                    DownloadStage::Done => "完成",
                };
                println!("[{stage}] {}", p.message);
            };
            install_model_to(&dest, force, &mut progress).map_err(|e| e.to_string())?;
            println!("TTS 主模型已就绪: {}", dest.display());
            install_vocoder_to(&dest, force, &mut progress).map_err(|e| e.to_string())?;
            println!("TTS 声码器已就绪: {}", dest.display());
            Ok(())
        }
    }
}

/// 读取 settings 并解析 TTS 配置
fn tts_config(
    cli_model_dir: Option<&PathBuf>,
) -> Result<crate::tts::config::ResolvedTtsConfig, String> {
    let settings = crate::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    crate::tts::config::resolve(tts_settings.as_ref(), cli_model_dir.map(|p| p.as_path()))
}

/// LLM 命令入口
async fn cmd_llm(cmd: LlmCmd) -> Result<(), String> {
    use crate::llm::provider::LlmProvider;
    use crate::llm::types::{ChatMessage, ChatRole, InputItem, OutputItem};
    use std::io::Write;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    match cmd {
        LlmCmd::Load { model_path } => {
            let cfg = llm_config(model_path.as_ref())?;
            let mut provider = crate::llm::local::LocalLlamaProvider::new(cfg.clone())
                .map_err(|e| e.to_string())?;
            provider.load().map_err(|e| e.to_string())?;
            println!("模型已加载: {}", cfg.model_path.display());
            println!("架构: {}", provider.architecture());
            println!("上下文: {} tokens", cfg.params.context_size);
            Ok(())
        }
        LlmCmd::Chat { text, model_path } => {
            let cfg = llm_config(model_path.as_ref())?;
            let mut provider = crate::llm::local::LocalLlamaProvider::new(cfg.clone())
                .map_err(|e| e.to_string())?;
            provider.load().map_err(|e| e.to_string())?;

            let input = vec![InputItem::Message(ChatMessage::new(ChatRole::User, text))];
            let cancel = Arc::new(AtomicBool::new(false));
            let mut emit = |item: OutputItem| {
                if let OutputItem::MessageDelta(delta) = item {
                    print!("{}", delta.text);
                    let _ = std::io::stdout().flush();
                }
            };
            let reason = provider
                .generate(&input, &[], &cfg.params, &mut emit, cancel)
                .map_err(|e| e.to_string())?;
            println!();
            println!("[生成结束: {reason:?}]");
            Ok(())
        }
    }
}

/// 读取 settings 并解析 LLM 配置
fn llm_config(
    cli_model_path: Option<&PathBuf>,
) -> Result<crate::llm::config::ResolvedLlmConfig, String> {
    let settings = crate::config::settings::load_settings()?;
    let llm_settings = settings.as_ref().and_then(|s| s.llm.clone());
    crate::llm::config::resolve(llm_settings.as_ref(), cli_model_path.map(|p| p.as_path()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constant() {
        assert!(!VERSION.is_empty(), "VERSION should not be empty");
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION should be in semver format (X.Y.Z)");
        for part in &parts {
            assert!(!part.is_empty(), "semver part should not be empty");
            assert!(
                part.chars().all(|c| c.is_ascii_digit()),
                "semver part '{}' should be numeric",
                part
            );
        }
    }

    #[test]
    fn test_config_output() {
        let output = cmd_config().unwrap();
        let val: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(val["debug"], serde_json::Value::Bool(false));
        assert_eq!(
            val["logLevel"],
            serde_json::Value::String("info".to_string())
        );
        assert_eq!(val.as_object().unwrap().len(), 3);
    }

    #[test]
    fn test_config_contains_version() {
        let output = cmd_config().unwrap();
        assert!(output.contains(VERSION));
    }

    #[test]
    fn test_greet_output() {
        // greet 直接打印到 stdout，验证不 panic
        cmd_greet("World", 1).expect("greet should succeed");
        cmd_greet("World", 0).expect("greet with 0 count should succeed");
    }

    #[test]
    fn test_completion_bash() {
        let mut buf = Vec::new();
        cmd_completion(clap_complete::Shell::Bash, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("complete -F"),
            "bash completion should contain complete -F"
        );
        for sub in &["config", "greet", "completion"] {
            assert!(
                output.contains(sub),
                "bash completion should contain subcommand {}",
                sub
            );
        }
    }

    #[test]
    fn test_completion_zsh() {
        let mut buf = Vec::new();
        cmd_completion(clap_complete::Shell::Zsh, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("#compdef"),
            "zsh completion should start with #compdef"
        );
        for sub in &["config", "greet", "completion"] {
            assert!(
                output.contains(sub),
                "zsh completion should contain subcommand {}",
                sub
            );
        }
    }

    #[test]
    fn test_completion_fish() {
        let mut buf = Vec::new();
        cmd_completion(clap_complete::Shell::Fish, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("complete -c"),
            "fish completion should contain complete -c"
        );
        for sub in &["config", "greet", "completion"] {
            assert!(
                output.contains(sub),
                "fish completion should contain subcommand {}",
                sub
            );
        }
    }

    #[test]
    fn test_completion_powershell() {
        let mut buf = Vec::new();
        cmd_completion(clap_complete::Shell::PowerShell, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Register-ArgumentCompleter"),
            "powershell completion should register argument completer"
        );
        for sub in &["config", "greet", "completion"] {
            assert!(
                output.contains(sub),
                "powershell completion should contain subcommand {}",
                sub
            );
        }
    }

    #[test]
    fn test_completion_all_shells_have_all_subcommands() {
        let shells = [
            clap_complete::Shell::Bash,
            clap_complete::Shell::Zsh,
            clap_complete::Shell::Fish,
            clap_complete::Shell::PowerShell,
        ];
        for shell in shells {
            let mut buf = Vec::new();
            cmd_completion(shell, &mut buf);
            let output = String::from_utf8(buf).unwrap();
            for sub in &["config", "greet", "completion"] {
                assert!(
                    output.contains(sub),
                    "{:?} completion should contain subcommand {}",
                    shell,
                    sub
                );
            }
        }
    }

    #[test]
    fn test_cli_parse_greet() {
        let cli = Cli::try_parse_from(&["test", "greet", "--name", "World"]).unwrap();
        match cli.command.unwrap() {
            Commands::Greet { name, count } => {
                assert_eq!(name, "World");
                assert_eq!(count, 1);
            }
            _ => panic!("Expected Greet command"),
        }
    }

    #[test]
    fn test_cli_parse_greet_with_count() {
        let cli = Cli::try_parse_from(&["test", "greet", "-n", "Test", "-c", "3"]).unwrap();
        match cli.command.unwrap() {
            Commands::Greet { name, count } => {
                assert_eq!(name, "Test");
                assert_eq!(count, 3);
            }
            _ => panic!("Expected Greet command"),
        }
    }

    #[test]
    fn test_cli_parse_config() {
        let cli = Cli::try_parse_from(&["test", "config"]).unwrap();
        assert!(matches!(cli.command.unwrap(), Commands::Config));
    }

    #[test]
    fn test_cli_parse_kws_test() {
        let cli = Cli::try_parse_from(&["test", "kws", "test"]).unwrap();
        match cli.command.unwrap() {
            Commands::Kws { cmd } => assert!(matches!(cmd, KwsCmd::Test { .. })),
            _ => panic!("Expected Kws command"),
        }
    }

    #[test]
    fn test_cli_parse_kws_run() {
        let cli = Cli::try_parse_from(&["test", "kws", "run", "--duration", "10"]).unwrap();
        match cli.command.unwrap() {
            Commands::Kws { cmd } => {
                assert!(matches!(
                    cmd,
                    KwsCmd::Run {
                        duration: Some(10),
                        ..
                    }
                ))
            }
            _ => panic!("Expected Kws command"),
        }
    }

    #[test]
    fn test_cli_parse_kws_devices() {
        let cli = Cli::try_parse_from(&["test", "kws", "devices"]).unwrap();
        match cli.command.unwrap() {
            Commands::Kws { cmd } => assert!(matches!(cmd, KwsCmd::Devices)),
            _ => panic!("Expected Kws command"),
        }
    }

    #[test]
    fn test_cli_parse_kws_install_model() {
        let cli = Cli::try_parse_from(&["test", "kws", "install-model", "--force"]).unwrap();
        match cli.command.unwrap() {
            Commands::Kws { cmd } => assert!(matches!(
                cmd,
                KwsCmd::InstallModel {
                    force: true,
                    model_dir: None
                }
            )),
            _ => panic!("Expected InstallModel command"),
        }
    }

    #[test]
    fn test_cli_parse_kws_install_model_with_dir() {
        let cli = Cli::try_parse_from(&["test", "kws", "install-model", "--model-dir", "/tmp/zm"])
            .unwrap();
        match cli.command.unwrap() {
            Commands::Kws { cmd } => assert!(matches!(
                cmd,
                KwsCmd::InstallModel {
                    model_dir: Some(_),
                    force: false
                }
            )),
            _ => panic!("Expected InstallModel command"),
        }
    }

    #[test]
    fn test_cli_parse_asr_test() {
        let cli = Cli::try_parse_from(&["test", "asr", "test"]).unwrap();
        match cli.command.unwrap() {
            Commands::Asr { cmd } => assert!(matches!(cmd, AsrCmd::Test { .. })),
            _ => panic!("Expected Asr command"),
        }
    }

    #[test]
    fn test_cli_parse_asr_run() {
        let cli = Cli::try_parse_from(&["test", "asr", "run", "--duration", "10"]).unwrap();
        match cli.command.unwrap() {
            Commands::Asr { cmd } => assert!(matches!(
                cmd,
                AsrCmd::Run {
                    duration: Some(10),
                    ..
                }
            )),
            _ => panic!("Expected Asr command"),
        }
    }

    #[test]
    fn test_cli_parse_asr_devices() {
        let cli = Cli::try_parse_from(&["test", "asr", "devices"]).unwrap();
        match cli.command.unwrap() {
            Commands::Asr { cmd } => assert!(matches!(cmd, AsrCmd::Devices)),
            _ => panic!("Expected Asr command"),
        }
    }

    #[test]
    fn test_cli_parse_asr_install_model() {
        let cli = Cli::try_parse_from(&["test", "asr", "install-model", "--force"]).unwrap();
        match cli.command.unwrap() {
            Commands::Asr { cmd } => assert!(matches!(
                cmd,
                AsrCmd::InstallModel {
                    force: true,
                    model_dir: None
                }
            )),
            _ => panic!("Expected InstallModel command"),
        }
    }

    #[test]
    fn test_cli_parse_tts_run() {
        let cli = Cli::try_parse_from(&["test", "tts", "run", "--text", "你好", "--speed", "1.2"])
            .unwrap();
        match cli.command.unwrap() {
            Commands::Tts { cmd } => assert!(matches!(
                cmd,
                TtsCmd::Run {
                    text,
                    speed: Some(1.2),
                    voice: None,
                    ..
                } if text == "你好"
            )),
            _ => panic!("Expected Tts command"),
        }
    }

    #[test]
    fn test_cli_parse_tts_run_with_voice() {
        let cli = Cli::try_parse_from(&[
            "test", "tts", "run", "--text", "你好", "--voice", "leijun-1",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Tts { cmd } => assert!(matches!(
                cmd,
                TtsCmd::Run {
                    voice: Some(id),
                    ..
                } if id == "leijun-1"
            )),
            _ => panic!("Expected Tts command"),
        }
    }

    #[test]
    fn test_cli_parse_tts_voices() {
        let cli = Cli::try_parse_from(&["test", "tts", "voices"]).unwrap();
        match cli.command.unwrap() {
            Commands::Tts { cmd } => assert!(matches!(cmd, TtsCmd::Voices { .. })),
            _ => panic!("Expected Tts command"),
        }
    }

    #[test]
    fn test_cli_parse_tts_install_model() {
        let cli = Cli::try_parse_from(&["test", "tts", "install-model", "--force"]).unwrap();
        match cli.command.unwrap() {
            Commands::Tts { cmd } => assert!(matches!(
                cmd,
                TtsCmd::InstallModel {
                    force: true,
                    model_dir: None
                }
            )),
            _ => panic!("Expected InstallModel command"),
        }
    }
}
