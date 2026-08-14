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
    },
    /// 离线转写 wav 文件（不需要麦克风）
    Test {
        /// wav 路径；默认 <model_dir>/test_wavs/0.wav
        #[arg(long)]
        wav: Option<PathBuf>,
        #[arg(long)]
        model_dir: Option<PathBuf>,
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
        } => {
            let cfg = asr_config(model_dir.as_ref())?;
            crate::asr::run_realtime(&cfg, device.as_deref(), duration)
        }
        AsrCmd::Test { wav, model_dir } => {
            let cfg = asr_config(model_dir.as_ref())?;
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
            use crate::asr::{DownloadProgress, DownloadStage, install_model_to, user_model_dir};
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

/// 读取 settings 并解析 ASR 配置
fn asr_config(
    cli_model_dir: Option<&PathBuf>,
) -> Result<crate::asr::config::ResolvedAsrConfig, String> {
    let settings = crate::config::settings::load_settings()?;
    let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
    crate::asr::config::resolve(asr_settings.as_ref(), cli_model_dir.map(|p| p.as_path()))
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
}
