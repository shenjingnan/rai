# @zapmomo-ai/dsh-plugin

dsh（[DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)）→ ZapMomo 桌宠的**任务事件桥**。

监听 dsh 的 `agent/status` 与 `session/event`（`turn/end`），把任务**开始 / 完成 / 失败 / 中断**翻译成语义化事件，POST 到 ZapMomo 的 loopback 桥（`POST /dsh/events`）。ZapMomo 桌宠据此以文字气泡 + 语音播报给用户反馈。

## 前置条件

- 已运行 **ZapMomo**（Tauri 应用，`[dsh]` 段默认启用）：ZapMomo 启动时在 `~/.zapmomo/runtime/dsh-bridge.json` 写桥端口与 Bearer token（权限 0600）
- 已安装 **deepseek-harness**（`dsh` CLI）

插件只在读到发现文件时才推送；ZapMomo 未运行时静默跳过（不报错、不影响 dsh）。

## 安装

```bash
# 从 npm（推荐）
dsh plugin --profile web add @zapmomo-ai/dsh-plugin

# 从源码（开发/调试，link 模式改动即时生效；<zapmomo> 换成你的仓库路径）
dsh plugin --profile web add link:<zapmomo>/integrations/dsh-plugin
```

装完因声明了 `dsh.bundle.patch`，插件**自动**进入 profile bundles，无需手动改 patch。重启 `dsh web` 生效。

## 行为

| dsh 事件 | 推送类型 | 备注 |
| --- | --- | --- |
| `agent/status` → `running` | `task-started` | title 取自会话最近一条 user/message（前 40 字符） |
| `turn/end` → `reason.kind = completed` | `task-finished` | |
| `turn/end` → `reason.kind = error` | `task-failed` | detail = `reason.error.message — code`（截 200） |
| `turn/end` → 其它（aborted/interrupted/max-tokens/blocked） | `task-interrupted` | |

推送纪律：每次发送前现读发现文件；POST 超时 1s、异常吞掉只打 debug——**绝不阻塞/影响 dsh 宿主**。

## 开发

```bash
# 发布新版本：改 version 后
npm publish
```

字段路径按 dsh 源码核对（`session.id` / `agent.status` / `reason.error`），如有 dsh 版本升级导致漂移，以 `src/index.ts` 顶部「待实测核对」注释为准回读对齐。
