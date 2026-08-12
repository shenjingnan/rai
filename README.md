# ai-rust-starter

一个开箱即用的 **Rust 项目快速启动模板**。

## 特性

- **CLI 骨架** — 基于 clap 的命令行参数解析，支持子命令和 Shell 补全生成
- **异步运行时** — 集成 tokio，开箱即用的 async/await 支持
- **配置管理** — TOML 格式的配置文件读写，支持 `${env.VAR}` 环境变量引用
- **双层日志** — 基于 tracing 的日志系统，同时输出到文件和 stderr
- **日期时间工具** — 基于 chrono 的常用时间格式转换函数
- **测试支持** — 集成 tempfile 的测试隔离辅助工具
- **代码质量** — cargo fmt / clippy / typos / tarpaulin / codecov 一站式配置
- **CI/CD** — GitHub Actions 自动化测试、发布、覆盖率报告
- **Shell 补全** — 支持 bash / zsh / fish / powershell 自动补全

## 快速开始

```bash
# 运行
cargo run
cargo run -- config
cargo run -- greet --name World

# 测试
cargo test

# 代码质量检查
cargo fmt --check
cargo clippy -- -D warnings
```

## 关键词唤醒词（KWS）

接入 sherpa-onnx 关键词检测模型（zipformer 中英混合），实现「说出唤醒词 → 程序反应」。

### 快速开始

```bash
# 1. 下载模型（约 31MB，存入 ./models/，不入库）
./scripts/download-kws-model.sh

# 2. 离线验证（无需麦克风）：对模型自带 wav 检测出「文森特卡索」「法国」
cargo run -- kws test

# 3. 实时监听：说出唤醒词，控制台打印反应（首次运行需授权麦克风）
cargo run -- kws run

# 4. 查看可用麦克风设备
cargo run -- kws devices
```

### 命令说明

| 命令 | 说明 |
|------|------|
| `kws run` | 实时监听麦克风，检测唤醒词。`--duration 秒` 限时、`--device 名称` 指定设备、`--keywords` 附加关键词 |
| `kws test` | 离线检测 wav（默认模型自带 `test_wavs/zh_3.wav`）。`--wav` 指定文件 |
| `kws devices` | 列出可用输入设备 |

### 配置

可在 `~/.ai-rust-starter/settings.toml` 中添加 `[kws]` 段覆盖默认值：

```toml
[kws]
model_dir = "/path/to/model"        # 模型目录（支持 ${env.VAR}）
num_threads = 4                      # 推理线程数，默认 2
chunk_size = 3200                    # 每次喂给模型的采样数（@16k），默认 3200
keywords_threshold = 0.25            # 触发阈值：越大越不容易误触发（0.15~0.5）
encoder = "encoder-...-chunk-16-left-64.int8.onnx"   # 可用 int8 变体
keywords_file = "/path/to/keywords.txt"              # 自定义关键词文件
```

### 自定义唤醒词

keywords 文件每行一个，格式为「拼音/音素 token + `@显示词`」：

```
w én s ēn t è k ǎ s uǒ @文森特卡索     # 中文：拼音首字母+带调韵母
L AY1 T AH1 P @LIGHT_UP                  # 英文：ARPAbet 音素
```

中文原始词转拼音 token 需 sherpa-onnx 的 `text2token --tokens-type ppinyin` 工具（Python CLI）。v1 默认使用模型自带的关键词集。

### 测试

```bash
# 常规测试（不依赖模型）
cargo test -- --test-threads=1

# 模型相关测试（需先下载模型）
./scripts/run-kws-model-tests.sh
```

## 项目结构

```
├── Cargo.toml           # 项目配置和依赖
├── rust-toolchain.toml  # Rust 工具链版本（1.85）
├── src/
│   ├── main.rs          # 入口文件
│   ├── lib.rs           # 库入口 + 测试工具
│   ├── cli.rs           # CLI 命令定义
│   ├── config/
│   │   ├── mod.rs       # 配置模块入口
│   │   └── settings.rs  # TOML 配置管理
│   ├── logging.rs       # tracing 双层日志
│   └── datetime.rs      # 日期时间工具
├── tests/               # 集成测试
├── .github/workflows/   # CI/CD
└── .githooks/           # Git hooks
```

## 使用此模板

1. 基于此仓库创建新项目
2. 全局搜索替换 `ai-rust-starter` 为你的项目名
3. 修改 `Cargo.toml` 中的项目元信息（name, version, description）
4. 按需调整依赖（`Cargo.toml` 中的可选依赖已注释说明）
5. 在 `src/cli.rs` 中定义你的命令
6. 开始编写业务代码

## 依赖说明

| 分类 | Crate | 用途 |
|------|-------|------|
| 核心 | clap | CLI 参数解析 |
| 核心 | tokio | 异步运行时 |
| 核心 | serde / serde_json / toml | 序列化 |
| 核心 | chrono | 日期时间处理 |
| 核心 | tracing / tracing-subscriber | 日志 |
| 核心 | thiserror / anyhow | 错误处理 |
| 可选 | reqwest | HTTP 客户端（按需引入） |

## 许可

[MIT](LICENSE)
