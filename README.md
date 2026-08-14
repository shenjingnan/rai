<div align="center">
  <img src="docs/public/logo.svg" alt="ZapMomo Logo" width="300" />
</div>

An open-source, real-time desktop **AI companion** with voice, memory, and a customizable virtual character.

开源的实时桌面 AI 伴侣：语音交互、记忆能力、可定制的虚拟角色。

> 📚 中文文档：[文档站](docs/)，含快速开始、KWS、配置、桌面应用与开发指南。

## 特性

- **语音唤醒（KWS）** — 基于 sherpa-onnx 的 zipformer 唤醒词检测，支持实时麦克风监听与离线 wav 检测；自定义关键词直接输中文，自动转拼音 token
- **桌面应用** — 基于 Tauri 2 的 GUI（KWS 控制面板），Windows / macOS / Linux 三平台安装包
- **音频采集** — 基于 cpal 的麦克风采集 + 自动重采样（设备采样率 → 16k）
- **CLI 骨架** — 基于 clap 的命令行参数解析，支持子命令和 Shell 补全生成
- **异步运行时** — 集成 tokio，开箱即用的 async/await 支持
- **配置管理** — TOML 格式的配置文件读写，支持 `${env.VAR}` 环境变量引用
- **双层日志** — 基于 tracing 的日志系统，同时输出到文件和 stderr
- **日期时间工具** — 基于 chrono 的常用时间格式转换函数
- **测试支持** — 集成 tempfile 的测试隔离辅助工具
- **代码质量** — cargo fmt / clippy / typos / tarpaulin / codecov 一站式配置
- **CI/CD** — GitHub Actions 自动化测试、发布、覆盖率报告
- **Shell 补全** — 支持 bash / zsh / fish / powershell / elvish 自动补全

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
# 1. 下载模型（约 31MB，默认安装到 ~/.zapmomo/models/<模型名>，不入库）
cargo run -- kws install-model

# 2. 离线验证（无需麦克风）：对模型自带 wav 检测出「文森特卡索」「法国」
cargo run -- kws test

# 3. 实时监听：说出唤醒词，控制台打印反应（首次运行需授权麦克风）
cargo run -- kws run

# 4. 查看可用麦克风设备
cargo run -- kws devices
```

### 模型来源与校验

模型**不随代码分发**，由 CLI 内置的 `kws install-model` 命令（或 `scripts/download-kws-model.sh`）按 `models/manifest.json` 清单下载：

- **清单** `models/manifest.json`（随仓库）记录每个模型的 `name / version / source / sha256 / license`
- **校验**：下载后对整包计算 sha256 与清单比对，**不匹配即删除报错**；解压先到临时目录再原子移动，避免留下损坏的半截模型
- **幂等**：模型已存在且完整则跳过
- **合规**：第三方来源与许可见 `models/THIRD_PARTY_NOTICES.md`
- ASR 已沿用同一套清单机制（见下文「语音识别」），后续 TTS 等模型亦将沿用

### 命令说明

| 命令 | 说明 |
|------|------|
| `kws run` | 实时监听麦克风，检测唤醒词。`--duration 秒` 限时、`--device 名称` 指定设备、`--keywords` 附加关键词（直接输中文，多个用 `/` 分隔） |
| `kws test` | 离线检测 wav（默认模型自带 `test_wavs/zh_3.wav`）。`--wav` 指定文件 |
| `kws devices` | 列出可用输入设备 |
| `kws install-model` | 下载并安装唤醒词模型（默认 `~/.zapmomo/models/<模型名>`）。`--model-dir` 指定目录、`--force` 强制重装 |

### 配置

可在 `~/.zapmomo/settings.toml` 中添加 `[kws]` 段覆盖默认值（全部可选）：

```toml
[kws]
model_dir = "/path/to/model"              # 模型目录（支持 ${env.VAR}）
provider = "cpu"                           # 推理后端，默认 cpu
num_threads = 4                            # 推理线程数，默认 2
chunk_size = 3200                          # 每次喂给模型的采样数（@16k），默认 3200
sample_rate = 16000                        # 模型输入采样率，默认 16000
keywords_score = 1.0                       # 关键词 boosting 分数
keywords_threshold = 0.25                  # 触发阈值：越大越不容易误触发（0.15~0.5）
encoder = "encoder-...-chunk-16-left-64.int8.onnx"   # 模型目录内带 int8 变体可选
decoder = "decoder-...-chunk-16-left-64.onnx"
joiner  = "joiner-...-chunk-16-left-64.onnx"
tokens  = "tokens.txt"
keywords_file = "/path/to/keywords.txt"    # 自定义关键词文件
debug = false
```

### 自定义唤醒词

**直接输入中文即可**：`--keywords 你好小智` 会由内置的拼音转换（`src/kws/token.rs`）自动把汉字拆成模型
可编码的 ppinyin token（`你好小智` → `n ǐ h ǎo x iǎo zh ì`），无需任何外部工具；多个关键词用 `/` 或换行分隔。

keywords 文件（默认 `<model_dir>/test_wavs/keywords.txt`，可用 `[kws] keywords_file` 覆盖）每行一个关键词，
同样支持直接写中文，也支持精确的「token + `@显示词`」格式：

```
你好小智 @你好小智                      # 中文：直接写，自动转 ppinyin
w én s ēn t è k ǎ s uǒ @文森特卡索     # 中文：精确 token（声母+带调韵母）
L AY1 T AH1 P @LIGHT_UP                  # 英文：ARPAbet 音素
```

v1 默认使用模型自带的中英混合关键词集（见 `test_wavs/keywords.txt`）。

### 测试

```bash
# 常规测试（不依赖模型）
cargo test -- --test-threads=1

# 模型相关测试（需先下载模型）
./scripts/run-kws-model-tests.sh
```

## 语音识别（ASR）

接入 sherpa-onnx 流式语音识别模型（zipformer 中英双语），把麦克风语音实时转成文本（支持中英混说）。

### 快速开始

```bash
# 1. 下载模型（约 500MB，int8 量化，默认安装到 ~/.zapmomo/models/<模型名>，不入库）
cargo run -- asr install-model

# 2. 离线转写（无需麦克风）：对模型自带 wav 输出转写文本
cargo run -- asr test

# 3. 实时转写：说话即出字幕（首次运行需授权麦克风，Ctrl-C 退出）
cargo run -- asr run

# 4. 查看可用麦克风设备
cargo run -- asr devices
```

模型来源、sha256 校验、幂等安装与 `[kws]` 完全一致（见上文「模型来源与校验」）。

### 命令说明

| 命令 | 说明 |
|------|------|
| `asr run` | 实时监听麦克风并转写。`--duration 秒` 限时、`--device 名称` 指定设备 |
| `asr test` | 离线转写 wav（默认模型自带 `test_wavs/0.wav`）。`--wav` 指定文件 |
| `asr devices` | 列出可用输入设备 |
| `asr install-model` | 下载并安装 ASR 模型（默认 `~/.zapmomo/models/<模型名>`）。`--model-dir` 指定目录、`--force` 强制重装 |

### 配置

可在 `~/.zapmomo/settings.toml` 中添加 `[asr]` 段覆盖默认值（全部可选）：

```toml
[asr]
model_dir = "/path/to/model"              # 模型目录（支持 ${env.VAR}）
provider = "cpu"                           # 推理后端，默认 cpu
num_threads = 4                            # 推理线程数，默认 2
decoding_method = "greedy_search"          # greedy_search | modified_beam_search
enable_endpoint = true                     # 端点检测（静音自动断句）
rule1_min_trailing_silence = 2.4          # 断句静音阈值（秒）
rule2_min_trailing_silence = 1.2
rule3_min_utterance_length = 20.0
encoder = "encoder-epoch-99-avg-1.int8.onnx"
decoder = "decoder-epoch-99-avg-1.onnx"    # 官方 int8 配方：fp32 decoder
joiner  = "joiner-epoch-99-avg-1.int8.onnx"
tokens  = "tokens.txt"
debug = false
```

## 桌面应用（Tauri 2）

复用同一套 KWS / 音频 / 配置逻辑的桌面 GUI：KWS 控制面板（选择麦克风、开始/停止监听、附加关键词直接输中文、实时显示检测结果、查看模型配置与缺失提示）。代码在 `src-tauri/`，前端为 React + Vite + TypeScript（Tailwind CSS + shadcn/ui，构建产物打包进应用）。

```bash
# 安装 Tauri CLI（首次）
pnpm install

# 开发模式（热重载，需已下载模型：cargo run -- kws install-model）
pnpm tauri dev

# 构建当前平台的安装包（macOS 产出 .app/.dmg）
pnpm tauri build
```

> 打包版内置「下载模型」按钮：首次监听时若缺模型，在「配置」面板点击即可自动
> 下载到 `~/.zapmomo/models/<模型名>`（也可用 `zapmomo kws install-model`）。
> macOS 未签名 dmg 首次打开若被 Gatekeeper 拦截，右键 →「打开」，或执行
> `xattr -dr com.apple.quarantine <应用路径>`。

## 发布流程

每次发布新版本会自动构建 **Windows / macOS（Intel+Apple Silicon）/ Linux** 安装包并合并到一个 GitHub Release：

1. 合入 `main` 后，`publish.yml` 中的 release-plz 自动 bump 版本、更新 changelog，打出 `vX.Y.Z` tag 并发布到 crates.io，同时维护「版本发布 PR」。
2. tag push 触发 `release.yml`：在三个平台的原生 runner 上运行 `tauri-action` 构建安装包（`.dmg` / `.app.tar.gz` / `.msi` / `.exe` / `.deb` / `.rpm` / `.AppImage`）。
3. 构建成功后自动发布为正式 Release（`draft: false`，不再停留在草稿）。

发布产物矩阵：

| 平台 | 安装包 |
|------|--------|
| macOS (Apple Silicon) | `.dmg` + `.app.tar.gz` |
| macOS (Intel) | `.dmg` + `.app.tar.gz` |
| Windows x64 | `.msi` + `.exe`（NSIS） |
| Linux x64 | `.deb` + `.rpm` + `.AppImage` |

> 签名：当前为未签名构建，适合内部/测试分发。正式对外发布时在仓库 Secrets 配置
> Apple Developer ID 证书（`APPLE_SIGNING_IDENTITY / APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID`）
> 与 Windows 证书后，tauri-action 会自动签名/公证。

## 项目结构

```
├── Cargo.toml           # 项目配置和依赖（workspace 根）
├── rust-toolchain.toml  # Rust 工具链版本（1.97.1）
├── src/
│   ├── main.rs          # 入口文件
│   ├── lib.rs           # 库入口 + 测试工具（test_util 临时 HOME 隔离）
│   ├── cli.rs           # CLI 命令定义
│   ├── config/
│   │   ├── mod.rs       # 配置模块入口
│   │   └── settings.rs  # TOML 配置管理（含 [kws] 段）
│   ├── kws/             # 关键词唤醒词检测（sherpa-onnx）
│   │   ├── mod.rs       # KwsEngine + 离线/实时检测
│   │   ├── config.rs    # KWS 配置解析与默认值
│   │   ├── model.rs     # 模型下载 / sha256 校验 / 解压安装
│   │   ├── token.rs     # 汉字 → ppinyin token 转换
│   │   └── reaction.rs  # Reaction 可插拔反应（控制台 / GUI / 测试）
│   ├── audio.rs         # cpal 麦克风采集 + 重采样
│   ├── logging.rs       # tracing 双层日志
│   └── datetime.rs      # 日期时间工具
├── models/              # 模型资产（本体不入库，按清单下载）
│   ├── manifest.json    # 模型清单（source / sha256 / license）
│   └── THIRD_PARTY_NOTICES.md
├── src-tauri/           # Tauri 2 桌面应用（workspace 成员）
│   ├── src/lib.rs       # commands + 监听线程 + TauriReaction
│   ├── frontend/        # React + Vite + TypeScript 控制面板（Tailwind + shadcn/ui）
│   ├── tauri.conf.json  # Tauri 配置（打包目标/图标/权限文案）
│   ├── capabilities/    # 权限声明
│   └── icons/           # 应用图标
├── tests/               # 集成测试
├── package.json         # Tauri CLI（@tauri-apps/cli）
├── scripts/             # 模型下载 / 模型测试 / 图标生成等脚本
├── .github/             # CI / 发布流水线 / Issue 模板
└── .githooks/           # Git hooks
```

## 依赖说明

| 分类 | Crate | 用途 |
|------|-------|------|
| 核心 | clap / clap_complete | CLI 参数解析 / Shell 补全生成 |
| 核心 | tokio | 异步运行时 |
| 核心 | serde / serde_json / toml | 序列化 |
| 核心 | chrono | 日期时间处理 |
| 核心 | tracing / tracing-subscriber | 日志 |
| 核心 | thiserror | 错误处理 |
| KWS | sherpa-onnx | 关键词唤醒词检测（zipformer，预编译库） |
| KWS | cpal | 麦克风音频采集 |
| KWS | pinyin | 汉字 → 带声调拼音（自定义关键词自动转换） |
| 模型下载 | ureq | HTTP 客户端（模型下载） |
| 模型下载 | sha2 / hex | 下载模型的 sha256 校验 |
| 模型下载 | tar / bzip2 | 解压 tar.bz2 模型包 |

## 许可

[MIT](LICENSE)
