# ai-rust-starter

一个开箱即用的 **Rust 项目快速启动模板**。

## 特性

- **CLI 骨架** — 基于 clap 的命令行参数解析，支持子命令和 Shell 补全生成
- **桌面应用** — 基于 Tauri 2 的 GUI 壳（KWS 控制面板），Windows / macOS / Linux 三平台安装包
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

### 模型来源与校验

模型**不随代码分发**，由 `scripts/download-kws-model.sh` 按 `models/manifest.json` 清单下载：

- **清单** `models/manifest.json`（随仓库）记录每个模型的 `name / version / source / sha256 / license`
- **校验**：下载后对整包计算 sha256 与清单比对，**不匹配即删除报错**；解压先到临时目录再原子移动，避免留下损坏的半截模型
- **幂等**：模型已存在且完整则跳过
- **合规**：第三方来源与许可见 `models/THIRD_PARTY_NOTICES.md`
- 后续 ASR / TTS 等模型（可能超过 100MB）将沿用同一套清单机制，按需下载

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

## 桌面应用（Tauri 2）

复用同一套 KWS / 音频 / 配置逻辑的桌面 GUI：KWS 控制面板（选择麦克风、开始/停止监听、实时显示检测结果、查看模型配置）。代码在 `src-tauri/`，前端为原生 HTML/CSS/JS（无构建链）。

```bash
# 安装 Tauri CLI（首次）
npm install

# 开发模式（热重载，需已下载模型：./scripts/download-kws-model.sh）
npm run tauri dev

# 构建当前平台的安装包（macOS 产出 .app/.dmg）
npm run tauri build
```

> 打包版的默认模型目录（`CARGO_MANIFEST_DIR` 烘焙）在用户机器上不存在，需在
> `~/.ai-rust-starter/settings.toml` 的 `[kws] model_dir` 指定模型位置；GUI 会提示。
> macOS 未签名 dmg 首次打开若被 Gatekeeper 拦截，右键 →「打开」，或执行
> `xattr -dr com.apple.quarantine <应用路径>`。

## 发布流程

每次发布新版本会自动构建 **Windows / macOS（Intel+Apple Silicon）/ Linux** 安装包并合并到一个 GitHub Release 草稿：

1. 合入 `main` 后，`publish.yml` 里的 release-plz 自动创建「版本发布 PR」（bump 版本 + 更新 changelog）。
2. 合并该 PR 后，release-plz 打出 `vX.Y.Z` tag 并发布到 crates.io。
3. tag push 触发 `release.yml`：在三个平台的原生 runner 上运行 `tauri-action` 构建安装包（`.dmg` / `.msi` / `.exe` / `.deb` / `.rpm` / `.AppImage`），统一附到一个**草稿 Release**。
4. 人工确认草稿后点击「发布」即为正式 Release。

发布产物矩阵：

| 平台 | 安装包 |
|------|--------|
| macOS (Apple Silicon) | `.dmg` |
| macOS (Intel) | `.dmg` |
| Windows x64 | `.msi` + `.exe`（NSIS） |
| Linux x64 | `.deb` + `.rpm` + `.AppImage` |

> 签名：当前为未签名构建，适合内部/测试分发。正式对外发布时在仓库 Secrets 配置
> Apple Developer ID 证书（`APPLE_SIGNING_IDENTITY / APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID`）
> 与 Windows 证书后，tauri-action 会自动签名/公证。

## 项目结构

```
├── Cargo.toml           # 项目配置和依赖（workspace 根）
├── rust-toolchain.toml  # Rust 工具链版本（1.88）
├── src/
│   ├── main.rs          # 入口文件
│   ├── lib.rs           # 库入口 + 测试工具
│   ├── cli.rs           # CLI 命令定义
│   ├── config/
│   │   ├── mod.rs       # 配置模块入口
│   │   └── settings.rs  # TOML 配置管理
│   ├── logging.rs       # tracing 双层日志
│   └── datetime.rs      # 日期时间工具
├── src-tauri/           # Tauri 2 桌面应用（workspace 成员）
│   ├── src/lib.rs       # commands + 监听线程 + TauriReaction
│   ├── frontend/        # 原生 HTML/CSS/JS 控制面板
│   ├── tauri.conf.json  # Tauri 配置（打包目标/图标/权限文案）
│   ├── capabilities/    # 权限声明
│   └── icons/           # 应用图标
├── tests/               # 集成测试
├── package.json         # Tauri CLI（@tauri-apps/cli）
├── scripts/             # 模型下载 / 图标生成等脚本
├── .github/workflows/   # CI / 发布流水线
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
