# 技术方案：Live2D 常驻透明窗口 + 设置窗口

## 一、背景与目标

当前 ZapMomo 桌面端只有一个窗口：KWS/ASR 控制面板（`App.tsx`），Live2D 模型仅作为设置面板里的「角色（Live2D）」卡片内嵌预览（`Live2dCard.tsx`）。

目标是把形态改成「桌面伴侣」模式：

- **角色窗口（companion）**：独立、透明背景、无边框、永远置顶的浮动窗口，常驻显示 Live2D 模型（仅静态展示：呼吸/眨眼等自动动画，不跟随鼠标）。
- **设置窗口（settings）**：现有控制面板，改由 `cmd+,`（macOS）或托盘菜单打开；关闭时隐藏而非退出进程。
- **常驻机制**：跨平台托盘图标 + 应用菜单 `cmd+,`，退出只经托盘/菜单（`Cmd+Q`）。

## 二、现状与架构分析

- `Live2dStage.tsx` 已用 `backgroundAlpha: 0` + `resolution: devicePixelRatio` 渲染透明画布，**渲染层已透明就绪**。
- `index.css` 已将 `html/body/#root` 设为透明，**CSS 已就绪**。
- `asset://` 协议 scope 在 `AppHandle` 级放行（`lib.rs`），**进程内所有窗口共享**。
- `tauri` 2.11.5 已具备多窗口（`WebviewWindowBuilder`）、菜单（`tauri::menu`，含 accelerator）、托盘（`TrayIconBuilder`）能力。
- 缺的只是「窗口透明 + 多窗口 + 常驻」这一层；Live2D 渲染与模型加载逻辑已完整复用。

## 三、目标架构

```mermaid
flowchart TB
    subgraph Process["Tauri 2 进程（单进程，两窗口共享状态 / asset scope）"]
        subgraph Companion["companion 窗口（常驻）"]
            CW["companion.html → companion.tsx<br/>→ CompanionRoot → Live2dStage"]
        end
        subgraph Settings["settings 窗口（按需打开）"]
            SW["settings.html → main.tsx → App.tsx<br/>（现有控制面板）"]
        end
        subgraph Native["原生层（lib.rs setup）"]
            W["WebviewWindowBuilder 创建两窗口"]
            M["应用菜单：偏好设置…(cmd+,) / 退出"]
            T["托盘图标：显示/隐藏角色 · 打开设置 · 退出"]
            E["事件：live2d-model-changed"]
        end
        W --> CW
        W --> SW
        M -->|cmd+,| SW
        T --> SW
        T -->|退出| Process
        SW -->|set_live2d_model| E
        E -->|重载模型| CW
    end
```

## 四、技术方案

### 4.1 核心决策

| 决策点 | 结论 |
|---|---|
| 常驻机制 | 托盘图标 + 应用菜单 `cmd+,`；关设置窗隐藏不退；退出走托盘/`Cmd+Q` |
| 窗口层级 | companion `always_on_top: true` |
| 前端结构 | 两个独立 Vite 入口：`companion.html`（轻量）+ `settings.html`（现有面板） |
| 交互范围 | 仅静态展示，`autoInteract: false` |
| 激活策略（macOS） | Phase 1 保持 Regular（保留 Dock 图标），确保 `cmd+,` 可靠工作 |
| 窗口创建方式 | 运行时在 `setup()` 用 `WebviewWindowBuilder` 创建 |

### 4.2 窗口配置

**companion**（所有平台一致）：`transparent` + `decorations:false` + `always_on_top` + `skip_taskbar` + `resizable:false` + `shadow:false`，固定内尺寸 `360×480`。

**settings**（保留现状，分平台）：macOS `title_bar_style(Overlay)` + `hidden_title` + `shadow` + 不透明 + `visible:false`；非 macOS `decorations:false` + `transparent` + CSS `rounded-xl`。

### 4.3 macOS 透明窗口的关键前提（macos-private-api）

`WebviewWindowBuilder::transparent()` 在 macOS 上被 `#[cfg(any(not(target_os = "macos"), feature = "macos-private-api"))]` 门控——**macOS 上让 WKWebView 背景透明需要私有 API**。因此需要同时：

1. `src-tauri/Cargo.toml`：`tauri` features 增加 `"macos-private-api"`（另有 `"tray-icon"`）。
2. `src-tauri/tauri.conf.json`：`app` 段增加 `"macOSPrivateApi": true`（Tauri 会校验 feature 与配置 allowlist 一致，否则构建脚本报错）。

`macos-private-api` 仅桌面应用可用（非 App Store），符合本项目定位。

### 4.4 后端改动（`src-tauri/src/lib.rs`）

- `setup()` 运行时创建两窗口；`cfg!(target_os = "macos")` 分支设置 settings 样式。
- 应用菜单：`MenuItem("show_settings", "偏好设置…", Some("CmdOrCtrl+,"))` + `PredefinedMenuItem::quit`，经 `app.set_menu`。
- 托盘：`TrayIconBuilder` + `default_window_icon()`，菜单「显示/隐藏角色 / 打开设置 / 退出」。
- `on_menu_event`（应用菜单）+ 托盘 `on_menu_event` 复用 `handle_menu(app, id)`。
- `on_window_event` 拦截 `CloseRequested` → `prevent_close` + `hide`（关窗不退进程）。
- `set_live2d_model` 成功后 `app.emit("live2d-model-changed", &info)`。

### 4.5 前端改动（`src-tauri/frontend`）

- Vite 多入口（`build.rollupOptions.input`）：`settings.html` + `companion.html`。
- 新入口 `companion.tsx` → `CompanionRoot.tsx`：挂载读 `get_live2d_config` 恢复模型（顺带重放行 asset scope），订阅 `live2d-model-changed` 即时重载，全窗渲染 `Live2dStage`（复用现有组件）。
- `lib/tauri.ts` 新增 `onLive2dModelChanged`。
- `App.tsx` 移除「macOS 首帧自动 show」逻辑（设置窗改为后端按需显示）。

### 4.6 权限（`capabilities/default.json`）

`windows` 由 `["main"]` 改为 `["companion", "settings"]`；权限沿用现有（`core:default`、窗口控制、`dialog:allow-open`）。

## 五、分阶段实施与验收

| 阶段 | 内容 | 验收 |
|---|---|---|
| 0 依赖骨架 | 加 feature、多入口、html 拆分、入口空壳 | `cargo check` / `pnpm build` 通过 |
| 1 多窗口透明 | 运行时创建两窗口、companion 渲染静态模型 | companion 透明置顶无边框显示模型 |
| 2 常驻入口 | 菜单/托盘/`cmd+,`、关闭拦截、事件 | `cmd+,` 开设置；关窗不退；托盘退出 |
| 3 同步打磨 | 换模型即时重载、拖拽移动、跨平台 | 全绿 + 三平台表现符合预期 |

## 六、验证方式

- `cargo fmt --check`、`cargo clippy -p zapmomo-app -- -D warnings`、`cargo check -p zapmomo-app`。
- `pnpm --dir src-tauri/frontend build`、`pnpm --dir src-tauri/frontend test:run`。
- `cargo test`。
- 端到端手测：`pnpm tauri dev` 核对透明/置顶/无边框、`cmd+,` 与托盘、关窗隐藏、换模型即时重载、托盘退出。

## 七、风险与决策记录

- **macOS 透明窗口**：需 `macos-private-api`（见 §4.3），避免与 `titleBarStyle: Overlay` 混用（仅 settings 用 Overlay）。
- **`cmd+,` vs 隐藏 Dock**：`cmd+,` 键盘加速键依赖 macOS 应用菜单栏，Phase 1 用 Regular 策略；隐藏 Dock 的 Accessory 模式留作后续可选。
- **Linux 透明**：WebKitGTK 透明可能不稳，保留不透明回退。
- **拖拽 vs 静态展示**：companion 全窗 `drag-region` 仅用于移动窗口；若与 PIXI canvas 有冲突，降级为托盘/菜单控制。
- **托盘图标深色模式**：Phase 1 用现有 `icon.png`，后续可替换为模板图。
