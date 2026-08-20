//! 伙伴库（Companion Library）：管理用户导入的 Live2D 模型集合与「当前使用」项。
//!
//! 数据模型：
//! - 清单：`~/.zapmomo/companions/library.json`（`CompanionLibrary`，schema_version = 1）
//! - 模型文件：`~/.zapmomo/companions/{id}/`（导入时把整个源目录复制进来，**应用托管**，
//!   源目录被删后伙伴仍可用）
//!
//! 导入采用「prepare（复制到临时目录 + 校验）→ commit（锁内二次去重 + 原子 rename）」，
//! 最终托管目录只经 `rename` 一次性出现，避免半成品目录；并发导入同一源时独立 tmp 目录，
//! 只有一个 `rename` 成功，其余返回 `already_imported`。
//!
//! 一致性：`CompanionLibrary.active_model_id` 是唯一逻辑 Source of Truth；
//! `settings.toml [live2d].model_dir` 只是兼容桌宠窗口的 derived runtime cache，
//! 由上层 `reconcile_active` 负责最终一致。

use crate::config::settings;
use crate::live2d::config as live2d_cfg;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// 伙伴库 schema 版本。
pub const SCHEMA_VERSION: u32 = 1;

const LIBRARY_FILE: &str = "library.json";
const TMP_PREFIX: &str = ".tmp-";
const ID_PREFIX: &str = "companion-";

/// 串行化 Library 的读改写与 commit 决策。**不得跨大型目录复制/删除持有。**
static COMPANION_LOCK: Mutex<()> = Mutex::new(());

/// 一个已导入的 Live2D 伙伴。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompanionModel {
    /// 稳定 id：`companion-` + sha256(canonical source 路径) 前 12 位。
    pub id: String,
    /// 角色名（导入目录 basename）。
    pub name: String,
    /// 原始导入目录（仅去重/记录来源；运行时绝不依赖，源删除后伙伴仍有效）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// 应用托管目录：`~/.zapmomo/companions/{id}`。
    pub model_dir: String,
    /// 托管目录内的 `.model3.json` 绝对路径。
    pub model_file: String,
    /// 模型格式（"cubism3"）。
    pub format: String,
    /// 导入时间（RFC3339）。
    pub imported_at: String,
}

/// 伙伴库清单（持久化到 `library.json`）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompanionLibrary {
    /// schema 版本；缺失字段按 v1（本版本首次发布，宽容处理）。
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub models: Vec<CompanionModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_model_id: Option<String>,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

impl Default for CompanionLibrary {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            models: Vec::new(),
            active_model_id: None,
        }
    }
}

/// 伙伴库根目录：`~/.zapmomo/companions`。
///
/// 这是**清单目录**（library.json 所在地），永远不跟随 `data_dir`；
/// 模型**载荷目录**见 `get_companions_store_dir`（可自定义）。
pub fn get_companions_dir() -> PathBuf {
    settings::get_settings_dir().join("companions")
}

/// 伙伴载荷存储根集合：当前 store 目录 + 旧默认根（去重）。
///
/// 供「拒绝导入已托管目录」检查与临时目录清理遍历——自定义 `data_dir` 后
/// 旧默认根 `~/.zapmomo/companions` 下的存量载荷仍属托管范围。
pub fn companion_store_roots() -> Vec<PathBuf> {
    let store = settings::get_companions_store_dir();
    let mut roots = vec![store];
    if let Some(legacy) = settings::legacy_companions_dir()
        && !roots.contains(&legacy)
    {
        roots.push(legacy);
    }
    roots
}

fn library_path() -> PathBuf {
    get_companions_dir().join(LIBRARY_FILE)
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    COMPANION_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// 读取库清单（**调用者必须已持有 COMPANION_LOCK**）。
///
/// - 文件不存在 → 正常返回空库（首次运行）。
/// - 存在但解析失败 / schema 高于支持版本 → 返回错误，**不覆盖原文件**。
fn load_library_inner() -> Result<CompanionLibrary, String> {
    let path = library_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CompanionLibrary::default());
        }
        Err(e) => return Err(format!("读取伙伴库失败: {e}")),
    };
    let lib: CompanionLibrary = serde_json::from_str(&content).map_err(|e| {
        format!(
            "伙伴库文件损坏（{}）：{e}，原文件已保留，请勿手动覆盖",
            path.display()
        )
    })?;
    if lib.schema_version > SCHEMA_VERSION {
        return Err(format!(
            "伙伴库版本 {} 高于当前 ZapMomo 支持的版本 {SCHEMA_VERSION}，请升级应用",
            lib.schema_version
        ));
    }
    Ok(lib)
}

/// 原子保存库清单（**调用者必须已持有 COMPANION_LOCK**）。
///
/// 「临时文件 + rename」模式，与 `tts/voice_store.rs` 一致；所有写库必须经此函数，
/// 禁止其它代码直接写 `library.json`。
fn save_library_inner(lib: &CompanionLibrary) -> Result<(), String> {
    let dir = get_companions_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建伙伴库目录失败: {e}"))?;
    let tmp = dir.join(format!("{LIBRARY_FILE}.tmp"));
    let content =
        serde_json::to_string_pretty(lib).map_err(|e| format!("序列化伙伴库失败: {e}"))?;
    std::fs::write(&tmp, content).map_err(|e| format!("写入伙伴库临时文件失败: {e}"))?;
    match std::fs::rename(&tmp, library_path()) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows：rename 无法覆盖已存在目标，先移除再重试。
            if library_path().exists() {
                std::fs::remove_file(library_path())
                    .map_err(|e| format!("移除旧伙伴库失败: {e}"))?;
            }
            std::fs::rename(&tmp, library_path()).map_err(|e| format!("保存伙伴库失败: {e}"))
        }
    }
}

/// 由规范化源绝对路径推导稳定 id。
pub fn derive_id(source_abs: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_abs.to_string_lossy().as_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{ID_PREFIX}{}", &hex[..12])
}

/// 快速有效判定：托管目录与清单文件都还在磁盘上（View 用，保持快速）。
pub fn quick_valid(model: &CompanionModel) -> bool {
    Path::new(&model.model_dir).is_dir() && Path::new(&model.model_file).is_file()
}

/// 从库中解析当前 active 伙伴（库应已 sanitize）。
pub fn active_model(lib: &CompanionLibrary) -> Option<&CompanionModel> {
    let active_id = lib.active_model_id.as_deref()?;
    lib.models.iter().find(|m| m.id.as_str() == active_id)
}

/// 校正 active：指向缺失/无效模型时落到第一个有效伙伴或 `None`；返回是否变更。
///
/// 有效判定使用轻量资源校验（`validate_managed_model`，只查元数据/存在性）。
fn sanitize_active_inner(lib: &mut CompanionLibrary) -> bool {
    if let Some(model) = active_model(lib)
        && live2d_cfg::validate_managed_model(Path::new(&model.model_dir)).is_ok()
    {
        return false;
    }
    let first_valid = lib
        .models
        .iter()
        .find(|m| live2d_cfg::validate_managed_model(Path::new(&m.model_dir)).is_ok())
        .map(|m| m.id.clone());
    if lib.active_model_id != first_valid {
        lib.active_model_id = first_valid;
        true
    } else {
        false
    }
}

/// 读取库并校正 active（不迁移旧版配置，毫秒级）。
///
/// 供启动阶段快速 reconcile 使用；若校正改变了 active 会持久化。
/// 顺带清理超过 1 小时的残留临时导入目录（崩溃中断遗留，best-effort）。
pub fn load_library_fast() -> Result<CompanionLibrary, String> {
    cleanup_stale_tmp_dirs();
    let _g = lock();
    let mut lib = load_library_inner()?;
    if sanitize_active_inner(&mut lib) {
        save_library_inner(&lib)?;
    }
    Ok(lib)
}

/// 读取库（空库时尝试迁移旧版 `[live2d].model_dir`）。
///
/// 迁移可能涉及大目录复制，因此空库分支会**释放锁**后再走 `migrate_legacy_if_empty`。
/// 供命令（`list_companions` 等）在 `spawn_blocking` 中调用。
pub fn load_library() -> Result<CompanionLibrary, String> {
    {
        let _g = lock();
        let lib = load_library_inner()?;
        if !lib.models.is_empty() {
            let mut l = lib;
            if sanitize_active_inner(&mut l) {
                save_library_inner(&l)?;
            }
            return Ok(l);
        }
    }
    // 空库：迁移失败仅告警，不影响返回空库（页面仍可用，下次自愈）。
    if let Err(e) = migrate_legacy_if_empty() {
        tracing::warn!("旧版模型迁移失败（将在下次打开伙伴页重试）: {e}");
    }
    load_library_fast()
}

// ===========================================================================
// 导入（prepare → commit）
// ===========================================================================

/// 导入准备结果：已存在（不复制）或就绪可提交。
enum Prepared {
    /// 同源（同 id）已导入。
    Already(CompanionModel),
    /// 已复制到临时目录并通过校验，待 commit。
    Ready(PreparedImport),
}

/// 已准备好、待原子提交的导入。
struct PreparedImport {
    id: String,
    name: String,
    source_path: String,
    tmp_dir: PathBuf,
    final_dir: PathBuf,
    /// `.model3.json` 相对托管根目录的路径。
    model_file_rel: PathBuf,
    format: String,
    imported_at: String,
}

/// RAII：退出作用域时清理残留临时目录（rename 成功后该路径已不存在，remove 为 no-op）。
struct TmpCleanup {
    path: PathBuf,
}

impl TmpCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TmpCleanup {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("清理临时目录失败: {}", e);
        }
    }
}

/// 生成唯一临时目录名（同进程内不冲突）。
fn new_tmp_dir_name(id: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{TMP_PREFIX}{id}-{millis}-{n}")
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// 从 tmp 目录名解析创建时间（毫秒）：`.tmp-{id}-{millis}-{n}`，其中 id = `companion-{hex}`。
fn tmp_dir_created_millis(name: &str) -> Option<u64> {
    let mut parts = name.strip_prefix(TMP_PREFIX)?.split('-');
    // 跳过 id 的两个段（"companion"、hex），第三段是毫秒时间戳。
    parts.next()?;
    parts.next()?;
    parts.next()?.parse().ok()
}

/// 清理残留的临时导入目录（best-effort）。
///
/// 仅清理创建时间超过 1 小时的目录，避免误删**正在进行的并发导入**的 tmp。
fn cleanup_stale_tmp_dirs() {
    const MAX_AGE_MS: u64 = 60 * 60 * 1000;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(u64::MAX);
    for root in companion_store_roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(millis) = tmp_dir_created_millis(&name) else {
                continue;
            };
            if now.saturating_sub(millis) > MAX_AGE_MS {
                let path = entry.path();
                if let Err(e) = std::fs::remove_dir_all(&path)
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::warn!("清理残留临时目录失败: {e}");
                }
            }
        }
    }
}

/// 递归复制目录。v1 忽略 symlink（打 warn），由托管副本校验兜底。
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("创建目标目录失败: {e}"))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("读取源目录失败: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败: {e}"))?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &target)
                .map_err(|e| format!("复制文件失败 ({}): {e}", entry.path().display()))?;
        } else if file_type.is_symlink() {
            tracing::warn!("导入时忽略符号链接: {}", entry.path().display());
        }
    }
    Ok(())
}

/// 导入准备（**不持锁**，可复制大目录）：
/// 校验源 → 去重预检 → 复制到唯一 tmp → 托管副本校验 → 计算相对清单路径。
fn prepare_import(source: &Path) -> Result<Prepared, String> {
    let source_abs = source
        .canonicalize()
        .map_err(|e| format!("无法访问源目录: {e}"))?;
    if !source_abs.is_dir() {
        return Err(format!("源路径不是目录: {}", source_abs.display()));
    }
    // 拒绝导入应用已托管的伙伴目录（防自我复制递归）。
    // 根目录也做 canonicalize：避免 macOS 上 /var → /private/var 这类符号链接
    // 使 canonicalize 后的源路径与未规范化根路径比较失配。
    // 自定义 data_dir 后旧默认根下的存量载荷仍属托管范围，两个根都查。
    for root in companion_store_roots() {
        let root_canon = root.canonicalize().unwrap_or_else(|_| root.clone());
        if source_abs.starts_with(&root_canon) {
            return Err("不能导入 ZapMomo 已托管的伙伴目录".to_string());
        }
    }

    let id = derive_id(&source_abs);
    let name = source_abs
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| id.clone());

    // 去重预检（短锁，避免已导入时白白复制大目录）。
    {
        let _g = lock();
        let lib = load_library_inner()?;
        if let Some(existing) = lib.models.iter().find(|m| m.id == id).cloned() {
            return Ok(Prepared::Already(existing));
        }
    }

    // 源目录结构校验（Cubism2 提前拒绝，避免复制后才发现）。
    let (_, source_format) = live2d_cfg::find_model_file(&source_abs)
        .ok_or_else(|| "目录中未找到 Live2D 模型清单（*.model3.json 或 model.json）".to_string())?;
    if source_format == live2d_cfg::Live2dFormat::Cubism2 {
        return Err(
            "暂不支持 Cubism 2 模型（.moc + model.json），请使用 Cubism 3/4/5 模型（.moc3 + .model3.json）"
                .to_string(),
        );
    }

    let store_dir = settings::get_companions_store_dir();
    let tmp_dir = store_dir.join(new_tmp_dir_name(&id));
    let final_dir = store_dir.join(&id);
    copy_dir_recursive(&source_abs, &tmp_dir)?;

    // 托管副本深度校验：Moc + Textures 必须存在。
    if let Err(e) = live2d_cfg::validate_managed_model(&tmp_dir) {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }

    // 记录 model3.json 相对 tmp 根的路径，commit 后换算成托管目录内的绝对路径。
    let (model_file_in_tmp, format) =
        live2d_cfg::find_model_file(&tmp_dir).ok_or_else(|| "复制后未找到模型清单".to_string())?;
    let model_file_rel = model_file_in_tmp
        .strip_prefix(&tmp_dir)
        .map_err(|_| "模型清单路径异常".to_string())?
        .to_path_buf();

    Ok(Prepared::Ready(PreparedImport {
        id,
        name,
        source_path: source_abs.display().to_string(),
        tmp_dir,
        final_dir,
        model_file_rel,
        format: format.to_str().to_string(),
        imported_at: now_rfc3339(),
    }))
}

/// 原子提交（加锁，锁只覆盖 rename/库更新/save 临界区）：
/// 二次去重 → rename → 更新库 → save；失败时清理残留，绝不留下「半成品目录」或「孤儿条目」。
fn commit_import(prepared: PreparedImport) -> Result<(CompanionModel, bool), String> {
    // 所有失败路径的残留 tmp 都由 RAII 清理（rename 成功后 tmp 已不存在，remove 为 no-op）。
    let _tmp = TmpCleanup::new(prepared.tmp_dir.clone());

    let lib = {
        let _g = lock();
        load_library_inner()?
    };
    // 去重（第一次，锁已释放）：同源已在库 → 已导入。
    if let Some(existing) = lib.models.iter().find(|m| m.id == prepared.id).cloned() {
        return Ok((existing, true));
    }

    let model = CompanionModel {
        id: prepared.id.clone(),
        name: prepared.name.clone(),
        source_path: Some(prepared.source_path.clone()),
        model_dir: prepared.final_dir.display().to_string(),
        model_file: prepared
            .final_dir
            .join(&prepared.model_file_rel)
            .display()
            .to_string(),
        format: prepared.format.clone(),
        imported_at: prepared.imported_at.clone(),
    };

    {
        let _g = lock();
        // 二次去重：并发导入同源可能在 prepare 与 commit 之间已插入。
        let lib2 = load_library_inner()?;
        if let Some(existing) = lib2.models.iter().find(|m| m.id == prepared.id).cloned() {
            return Ok((existing, true));
        }
        // 原子 rename（同卷）：最终托管目录只经 rename 一次性出现。
        if let Err(e) = std::fs::rename(&prepared.tmp_dir, &prepared.final_dir) {
            return Err(format!("提交托管目录失败: {e}"));
        }
        let first = lib2.models.is_empty();
        let mut new_lib = lib2;
        new_lib.models.push(model.clone());
        if first {
            new_lib.active_model_id = Some(model.id.clone());
        }
        if let Err(e) = save_library_inner(&new_lib) {
            // 库未持久化成功：先结束临界区，再移除已 rename 的最终目录防 orphan。
            drop(_g);
            if let Err(rm_e) = std::fs::remove_dir_all(&prepared.final_dir) {
                tracing::warn!("移除托管目录失败: {rm_e}");
            }
            return Err(e);
        }
    }
    Ok((model, false))
}

/// 导入 Live2D 模型目录：复制到应用托管目录并登记进伙伴库。
///
/// 返回 `(CompanionModel, already_imported)`；首次导入自动设为 active。
pub fn import_from_dir(source: &Path) -> Result<(CompanionModel, bool), String> {
    match prepare_import(source)? {
        Prepared::Already(model) => Ok((model, true)),
        Prepared::Ready(prepared) => commit_import(prepared),
    }
}

// ===========================================================================
// active / 迁移
// ===========================================================================

/// 设置当前使用伙伴（Library 先持久化成功，再由上层 reconcile 同步桌宠）。
///
/// 会做轻量资源校验：模型必须可基础加载。
pub fn set_active(id: &str) -> Result<CompanionLibrary, String> {
    let lib;
    {
        let _g = lock();
        let mut inner = load_library_inner()?;
        let model = inner
            .models
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| "未找到该伙伴".to_string())?;
        live2d_cfg::validate_managed_model(Path::new(&model.model_dir))
            .map_err(|e| format!("该伙伴模型不可用，无法设为当前使用：{e}"))?;
        inner.active_model_id = Some(id.to_string());
        save_library_inner(&inner)?;
        lib = inner;
    }
    Ok(lib)
}

/// 移除伙伴：删除托管目录 + 库条目；若删的是 active，active 自动落到第一个有效伙伴
/// 或 `None`。先保存库清单（事实来源），再 best-effort 删除托管目录。
pub fn remove(id: &str) -> Result<CompanionLibrary, String> {
    let (lib, removed_dir) = {
        let _g = lock();
        let mut inner = load_library_inner()?;
        let Some(idx) = inner.models.iter().position(|m| m.id == id) else {
            return Err("未找到该伙伴".to_string());
        };
        let model = inner.models.remove(idx);
        // 删的是 active → 落到第一个有效伙伴（quick_valid 足够，load 时的 sanitize 会再深校验）。
        if inner.active_model_id.as_deref() == Some(id) {
            inner.active_model_id = inner
                .models
                .iter()
                .find(|m| quick_valid(m))
                .map(|m| m.id.clone());
        }
        save_library_inner(&inner)?;
        (inner, Some(PathBuf::from(&model.model_dir)))
    };
    // 解锁后 best-effort 删除托管目录（失败仅告警，不留下「有库条目但目录缺失」的悬空状态）。
    if let Some(dir) = removed_dir
        && let Err(e) = std::fs::remove_dir_all(&dir)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("删除托管目录失败: {e}");
    }
    Ok(lib)
}

/// 把前端从 Live2D 渲染画布截取的 PNG 写入伙伴托管目录 `{id}/cover.png`。
///
/// `cover_image` 由视图层 `find_cover_image` 探测，无需改库结构。
pub fn save_cover(id: &str, png: &[u8]) -> Result<(), String> {
    let model_dir = {
        let _g = lock();
        let lib = load_library_inner()?;
        let model = lib
            .models
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| "未找到该伙伴".to_string())?;
        PathBuf::from(&model.model_dir)
    };
    std::fs::create_dir_all(&model_dir).map_err(|e| format!("创建托管目录失败: {e}"))?;
    let tmp = model_dir.join("cover.png.tmp");
    std::fs::write(&tmp, png).map_err(|e| format!("写入封面图失败: {e}"))?;
    std::fs::rename(&tmp, model_dir.join("cover.png")).map_err(|e| format!("保存封面图失败: {e}"))
}

/// 伙伴展示名最大长度。
const MAX_NAME_CHARS: usize = 30;

/// 重命名伙伴（只改展示名，**不触碰托管文件**，不影响 active / 桌宠）。
pub fn rename(id: &str, name: &str) -> Result<CompanionLibrary, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("伙伴名称不能为空".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("伙伴名称过长（最多 {MAX_NAME_CHARS} 个字符）").to_string());
    }
    let lib;
    {
        let _g = lock();
        let mut inner = load_library_inner()?;
        let model = inner
            .models
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or_else(|| "未找到该伙伴".to_string())?;
        model.name = name.to_string();
        save_library_inner(&inner)?;
        lib = inner;
    }
    Ok(lib)
}

/// 迁移后改写伙伴库中某条目的载荷路径（`model_dir`/`model_file` 指向新 store 目录）。
///
/// 旧载荷根 = 条目当前路径所在的管理根（`companion_store_roots()` 中能剥离的那个）。
/// 幂等：`id` 不存在时返回 `Ok(None)`。持 `COMPANION_LOCK`，锁不跨大文件复制。
pub fn relocate_payload(id: &str, new_dir: &Path) -> Result<Option<CompanionModel>, String> {
    let _g = lock();
    let mut lib = load_library_inner()?;
    let relocated = {
        let Some(model) = lib.models.iter_mut().find(|m| m.id == id) else {
            return Ok(None);
        };
        // 旧载荷根：条目当前路径能剥离的那个管理根
        let old_root = companion_store_roots()
            .into_iter()
            .find(|r| settings::strip_prefix_ci(Path::new(&model.model_dir), r).is_some())
            .unwrap_or_else(get_companions_dir);
        model.model_dir = relocate_in_root(&model.model_dir, &old_root, new_dir);
        model.model_file = relocate_in_root(&model.model_file, &old_root, new_dir);
        model.clone()
    }; // 可变借用在此结束
    save_library_inner(&lib)?;
    Ok(Some(relocated))
}

/// 把 `path` 从 `old_root` 前缀改写为 `new_root`（不在 old_root 下则原样返回）。
fn relocate_in_root(path: &str, old_root: &Path, new_root: &Path) -> String {
    let p = Path::new(path);
    match settings::strip_prefix_ci(p, old_root) {
        Some(rest) => new_root.join(rest).display().to_string(),
        None => path.to_string(),
    }
}

/// 旧版迁移：库为空且 `[live2d].model_dir` 指向合法模型时，复用安全导入流程复制进
/// 托管目录并设为 active。返回新伙伴 id；无需迁移返回 `None`。
///
/// 幂等（库非空跳过）；由启动后台任务调用，`load_library` 亦会兜底触发。
pub fn migrate_legacy_if_empty() -> Result<Option<String>, String> {
    let legacy_dir: Option<PathBuf> = {
        let _g = lock();
        let lib = load_library_inner()?;
        if !lib.models.is_empty() {
            return Ok(None);
        }
        settings::load_settings()?
            .and_then(|s| s.live2d)
            .and_then(|l| l.model_dir)
            .map(PathBuf::from)
    };
    let Some(dir) = legacy_dir else {
        return Ok(None);
    };
    if live2d_cfg::find_model_file(&dir).is_none() {
        return Ok(None);
    }
    let (model, _already) = import_from_dir(&dir)?;
    Ok(Some(model.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::run_with_temp_home;

    /// 构造一个校验可通过的模型目录。
    fn make_valid_model(dir: &Path, manifest: &str) {
        std::fs::create_dir_all(dir.join("textures")).unwrap();
        std::fs::write(dir.join("model.moc3"), b"moc").unwrap();
        std::fs::write(dir.join("textures/texture_00.png"), b"png").unwrap();
        std::fs::write(
            dir.join(manifest),
            r#"{"FileReferences":{"Moc":"model.moc3","Textures":["textures/texture_00.png"]}}"#,
        )
        .unwrap();
    }

    #[test]
    fn test_derive_id_stable_and_distinct() {
        run_with_temp_home(|home| {
            let a = home.join("A");
            let b = home.join("B");
            assert_eq!(derive_id(&a), derive_id(&a));
            assert_ne!(derive_id(&a), derive_id(&b));
            assert!(derive_id(&a).starts_with(ID_PREFIX));
        });
    }

    #[test]
    fn test_default_library_schema_version() {
        let lib = CompanionLibrary::default();
        assert_eq!(lib.schema_version, SCHEMA_VERSION);
        assert!(lib.models.is_empty());
        assert!(lib.active_model_id.is_none());
    }

    #[test]
    fn test_load_library_missing_file_returns_empty() {
        run_with_temp_home(|_home| {
            let lib = load_library_fast().unwrap();
            assert!(lib.models.is_empty());
        });
    }

    #[test]
    fn test_import_goes_to_custom_store_dir() {
        run_with_temp_home(|home| {
            // 设置自定义数据目录
            let data = home.join("zapdata");
            let mut config = settings::AppConfig::default();
            config.data_dir = Some(data.display().to_string());
            settings::save_settings(&config).unwrap();

            let src = home.join("srcmodel");
            make_valid_model(&src, "srcmodel.model3.json");
            let (model, _already) = import_from_dir(&src).unwrap();

            // 载荷目录在 <data_dir>/companions 下
            let store = data.join("companions");
            assert!(model.model_dir.starts_with(&store.display().to_string()));
            assert!(Path::new(&model.model_dir).is_dir());
            assert!(Path::new(&model.model_file).is_file());
            // 清单 library.json 仍留在 ~/.zapmomo/companions（不跟随 data_dir）
            assert!(home.join(".zapmomo/companions/library.json").is_file());
            // 旧默认载荷根目录下没有该载荷
            assert!(!home.join(".zapmomo/companions").join(&model.id).exists());
        });
    }

    #[test]
    fn test_relocate_payload_rewrites_paths() {
        run_with_temp_home(|home| {
            let data = crate::test_util::set_custom_data_dir(home);
            let old_root = home.join(".zapmomo/companions");
            let id = "companion-abc";
            // 造一个已导入的伙伴：旧根目录 + library.json
            make_valid_model(&old_root.join(id), "cat.model3.json");
            let lib = CompanionLibrary {
                schema_version: SCHEMA_VERSION,
                models: vec![CompanionModel {
                    id: id.to_string(),
                    name: "cat".into(),
                    source_path: None,
                    model_dir: old_root.join(id).display().to_string(),
                    model_file: old_root
                        .join(id)
                        .join("cat.model3.json")
                        .display()
                        .to_string(),
                    format: "Cubism3".into(),
                    imported_at: "t".into(),
                }],
                active_model_id: Some(id.to_string()),
            };
            std::fs::create_dir_all(&old_root).unwrap();
            let json = serde_json::to_string_pretty(&lib).unwrap();
            std::fs::write(old_root.join(LIBRARY_FILE), json).unwrap();

            let new_store = data.join("companions");
            let updated = relocate_payload(id, &new_store).unwrap().unwrap();
            assert_eq!(updated.model_dir, new_store.join(id).display().to_string());
            assert_eq!(
                updated.model_file,
                new_store
                    .join(id)
                    .join("cat.model3.json")
                    .display()
                    .to_string()
            );
            // 已持久化到 library.json
            let reloaded = load_library_fast().unwrap();
            let m = reloaded.models.iter().find(|m| m.id == id).unwrap();
            assert_eq!(m.model_dir, new_store.join(id).display().to_string());
        });
    }

    #[test]
    fn test_import_rejects_legacy_default_root_when_custom() {
        run_with_temp_home(|home| {
            let data = home.join("zapdata");
            let mut config = settings::AppConfig::default();
            config.data_dir = Some(data.display().to_string());
            settings::save_settings(&config).unwrap();

            // 源目录在旧默认根下（迁移前的载荷位置）：同样视为已托管，拒绝导入
            let inside_legacy = home.join(".zapmomo/companions/companion-abc");
            make_valid_model(&inside_legacy, "m.model3.json");
            let err = import_from_dir(&inside_legacy).unwrap_err();
            assert!(err.contains("已托管"));
        });
    }

    #[test]
    fn test_import_first_model_sets_active_and_copies() {
        run_with_temp_home(|home| {
            let src = home.join("大月下");
            make_valid_model(&src, "大月下.model3.json");
            let (model, already) = import_from_dir(&src).unwrap();
            assert!(!already);
            assert!(model.name == "大月下");
            // 托管副本已复制：model_dir / model_file 均指向 companions 下，且文件存在。
            let expected_dir = get_companions_dir().join(&model.id);
            assert_eq!(model.model_dir, expected_dir.display().to_string());
            assert!(Path::new(&model.model_file).is_file());
            assert!(model.model_file.starts_with(&model.model_dir));
            // 首次导入自动 active。
            let lib = load_library_fast().unwrap();
            assert_eq!(lib.active_model_id.as_deref(), Some(model.id.as_str()));
            assert_eq!(lib.models.len(), 1);
            // 源目录被删后仍 valid（托管副本）。
            std::fs::remove_dir_all(&src).unwrap();
            assert!(quick_valid(&model));
        });
    }

    #[test]
    fn test_import_second_model_keeps_active() {
        run_with_temp_home(|home| {
            let a = home.join("A");
            make_valid_model(&a, "a.model3.json");
            let (model_a, _) = import_from_dir(&a).unwrap();

            let b = home.join("B");
            make_valid_model(&b, "b.model3.json");
            let (model_b, already) = import_from_dir(&b).unwrap();
            assert!(!already);
            let lib = load_library_fast().unwrap();
            // active 仍是 A，B 只是加入列表。
            assert_eq!(lib.active_model_id.as_deref(), Some(model_a.id.as_str()));
            assert_eq!(lib.models.len(), 2);
            assert!(lib.models.iter().any(|m| m.id == model_b.id));
        });
    }

    #[test]
    fn test_reimport_same_source_is_already_imported() {
        run_with_temp_home(|home| {
            let src = home.join("同款");
            make_valid_model(&src, "m.model3.json");
            let (first, _) = import_from_dir(&src).unwrap();
            let (second, already) = import_from_dir(&src).unwrap();
            assert!(already);
            assert_eq!(first.id, second.id);
            assert_eq!(load_library_fast().unwrap().models.len(), 1);
        });
    }

    #[test]
    fn test_import_rejects_missing_manifest() {
        run_with_temp_home(|home| {
            let src = home.join("empty");
            std::fs::create_dir_all(&src).unwrap();
            let err = import_from_dir(&src).unwrap_err();
            assert!(err.contains("未找到 Live2D 模型清单"));
        });
    }

    #[test]
    fn test_import_rejects_cubism2() {
        run_with_temp_home(|home| {
            let src = home.join("c2");
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(src.join("model.json"), "{}").unwrap();
            let err = import_from_dir(&src).unwrap_err();
            assert!(err.contains("Cubism 2"));
        });
    }

    #[test]
    fn test_import_rejects_inside_companions_root() {
        run_with_temp_home(|_home| {
            let root = get_companions_dir();
            std::fs::create_dir_all(&root).unwrap();
            let inside = root.join("companion-abc123456789");
            std::fs::create_dir_all(&inside).unwrap();
            let err = import_from_dir(&inside).unwrap_err();
            assert!(err.contains("已托管"));
        });
    }

    #[test]
    fn test_import_fails_cleanly_on_managed_validation() {
        run_with_temp_home(|home| {
            let src = home.join("bad");
            std::fs::create_dir_all(src.join("textures")).unwrap();
            std::fs::write(src.join("textures/texture_00.png"), b"png").unwrap();
            // Moc 缺失 → 托管校验失败。
            std::fs::write(
                src.join("bad.model3.json"),
                r#"{"FileReferences":{"Moc":"missing.moc3","Textures":["textures/texture_00.png"]}}"#,
            )
            .unwrap();
            let err = import_from_dir(&src).unwrap_err();
            assert!(err.contains("Moc"), "{err}");
            // 无最终目录、无库条目、无残留 tmp。
            let root = get_companions_dir();
            assert!(!root.join("companion-").exists());
            let stray = std::fs::read_dir(&root)
                .map(|mut it| {
                    it.any(|e| {
                        e.unwrap()
                            .file_name()
                            .to_string_lossy()
                            .starts_with(TMP_PREFIX)
                    })
                })
                .unwrap_or(false);
            assert!(!stray, "不应残留 tmp 目录");
            assert!(load_library_fast().unwrap().models.is_empty());
        });
    }

    #[test]
    fn test_corrupt_library_returns_error_and_keeps_file() {
        run_with_temp_home(|_home| {
            let dir = get_companions_dir();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(LIBRARY_FILE), "{not-json").unwrap();
            let err = load_library_fast().unwrap_err();
            assert!(err.contains("损坏"), "{err}");
            // 原文件未被覆盖。
            let content = std::fs::read_to_string(dir.join(LIBRARY_FILE)).unwrap();
            assert_eq!(content, "{not-json");
        });
    }

    #[test]
    fn test_library_unsupported_schema_version() {
        run_with_temp_home(|_home| {
            let dir = get_companions_dir();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(LIBRARY_FILE),
                r#"{"schema_version":999,"models":[],"active_model_id":null}"#,
            )
            .unwrap();
            let err = load_library_fast().unwrap_err();
            assert!(err.contains("高于"), "{err}");
            // 不覆盖。
            let content = std::fs::read_to_string(dir.join(LIBRARY_FILE)).unwrap();
            assert!(content.contains("999"));
        });
    }

    #[test]
    fn test_library_schema_missing_defaults_to_v1() {
        run_with_temp_home(|_home| {
            let dir = get_companions_dir();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(LIBRARY_FILE),
                r#"{"models":[],"active_model_id":null}"#,
            )
            .unwrap();
            let lib = load_library_fast().unwrap();
            assert_eq!(lib.schema_version, SCHEMA_VERSION);
        });
    }

    #[test]
    fn test_set_active_validates_model() {
        run_with_temp_home(|home| {
            let src = home.join("A");
            make_valid_model(&src, "a.model3.json");
            let (model, _) = import_from_dir(&src).unwrap();

            let b = home.join("B");
            make_valid_model(&b, "b.model3.json");
            let (model_b, _) = import_from_dir(&b).unwrap();

            // 把 A 的托管目录删掉 → A 不可用。
            std::fs::remove_dir_all(&model.model_dir).unwrap();

            // 设置 A 为 active 应失败（模型不可用）。
            let err = set_active(&model.id).unwrap_err();
            assert!(err.contains("不可用"), "{err}");

            // B 正常设为 active。
            set_active(&model_b.id).unwrap();
            assert_eq!(
                load_library_fast().unwrap().active_model_id.as_deref(),
                Some(model_b.id.as_str())
            );
        });
    }

    #[test]
    fn test_rename_companion() {
        run_with_temp_home(|home| {
            let src = home.join("旧名");
            make_valid_model(&src, "m.model3.json");
            let (model, _) = import_from_dir(&src).unwrap();

            rename(&model.id, "  新名字  ").unwrap(); // trim 后生效
            let lib = load_library_fast().unwrap();
            assert_eq!(lib.models[0].name, "新名字");
            assert_eq!(lib.models[0].id, model.id);
            // 只改名字，不碰托管文件与 active。
            assert_eq!(lib.active_model_id.as_deref(), Some(model.id.as_str()));
            assert!(Path::new(&model.model_dir).is_dir());

            // 空名称 / 超长拒绝。
            assert!(rename(&model.id, "   ").is_err());
            assert!(rename(&model.id, &"很".repeat(MAX_NAME_CHARS + 1)).is_err());
            // 不存在的 id 拒绝。
            assert!(rename("companion-nope123", "x").is_err());
        });
    }

    #[test]
    fn test_remove_companion() {
        run_with_temp_home(|home| {
            let a = home.join("A");
            make_valid_model(&a, "a.model3.json");
            let (model_a, _) = import_from_dir(&a).unwrap();
            let b = home.join("B");
            make_valid_model(&b, "b.model3.json");
            let (model_b, _) = import_from_dir(&b).unwrap();

            // 删除 active A → 托管目录删除、active 落到 B。
            remove(&model_a.id).unwrap();
            let lib = load_library_fast().unwrap();
            assert_eq!(lib.models.len(), 1);
            assert_eq!(lib.active_model_id.as_deref(), Some(model_b.id.as_str()));
            assert!(!Path::new(&model_a.model_dir).exists(), "托管目录应被删除");

            // 删除不存在的 id 报错。
            assert!(remove("companion-nope123").is_err());

            // 删除最后一个 → active 置空。
            remove(&model_b.id).unwrap();
            let lib = load_library_fast().unwrap();
            assert!(lib.models.is_empty());
            assert!(lib.active_model_id.is_none());
        });
    }

    #[test]
    fn test_save_cover_writes_png_and_is_detected() {
        run_with_temp_home(|home| {
            let src = home.join("A");
            make_valid_model(&src, "a.model3.json");
            let (model, _) = import_from_dir(&src).unwrap();

            save_cover(&model.id, b"\x89PNG\r\n\x1a\n fake-png-bytes").unwrap();
            let cover = PathBuf::from(&model.model_dir).join("cover.png");
            assert!(cover.is_file(), "封面应写入托管目录");
            // find_cover_image 应探测到生成的 cover.png。
            let found = live2d_cfg::find_cover_image(Path::new(&model.model_dir)).unwrap();
            assert_eq!(found, cover);

            // 不存在的 id 报错，不写文件。
            assert!(save_cover("companion-nope", b"x").is_err());
        });
    }

    #[test]
    fn test_sanitize_falls_back_to_first_valid() {
        run_with_temp_home(|home| {
            let a = home.join("A");
            make_valid_model(&a, "a.model3.json");
            let (model_a, _) = import_from_dir(&a).unwrap();

            let b = home.join("B");
            make_valid_model(&b, "b.model3.json");
            let (model_b, _) = import_from_dir(&b).unwrap();

            // 把 active（A）的托管目录删掉 → load 后 sanitize 应落到 B。
            std::fs::remove_dir_all(&model_a.model_dir).unwrap();
            let lib = load_library_fast().unwrap();
            assert_eq!(lib.active_model_id.as_deref(), Some(model_b.id.as_str()));
        });
    }

    #[test]
    fn test_sanitize_clears_active_when_all_invalid() {
        run_with_temp_home(|home| {
            let src = home.join("A");
            make_valid_model(&src, "a.model3.json");
            let (model, _) = import_from_dir(&src).unwrap();
            std::fs::remove_dir_all(&model.model_dir).unwrap();
            let lib = load_library_fast().unwrap();
            assert!(lib.active_model_id.is_none());
        });
    }

    #[test]
    fn test_migrate_legacy_imports_and_is_idempotent() {
        run_with_temp_home(|home| {
            // 模拟旧版配置：settings.toml [live2d].model_dir 指向外部模型目录。
            let legacy = home.join("旧模型");
            make_valid_model(&legacy, "旧模型.model3.json");
            let config = settings::AppConfig {
                live2d: Some(settings::Live2dSettings {
                    model_dir: Some(legacy.display().to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            settings::save_settings(&config).unwrap();

            let migrated = migrate_legacy_if_empty().unwrap();
            assert!(migrated.is_some());
            let lib = load_library_fast().unwrap();
            assert_eq!(lib.models.len(), 1);
            assert_eq!(lib.active_model_id, migrated);

            // 幂等：库非空后不再迁移。
            assert!(migrate_legacy_if_empty().unwrap().is_none());
        });
    }

    #[test]
    fn test_load_library_triggers_migration_when_empty() {
        run_with_temp_home(|home| {
            let legacy = home.join("旧模型");
            make_valid_model(&legacy, "旧.model3.json");
            let config = settings::AppConfig {
                live2d: Some(settings::Live2dSettings {
                    model_dir: Some(legacy.display().to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            settings::save_settings(&config).unwrap();

            // load_library（含迁移）能完成，且无死锁。
            let lib = load_library().unwrap();
            assert_eq!(lib.models.len(), 1);
            assert!(lib.active_model_id.is_some());
        });
    }

    #[test]
    fn test_tmp_dir_created_millis_parses() {
        assert_eq!(
            tmp_dir_created_millis(".tmp-companion-abcdef012345-1700000000000-3"),
            Some(1_700_000_000_000)
        );
        assert!(tmp_dir_created_millis(".tmp-companion").is_none());
        assert!(tmp_dir_created_millis("companion-abcdef012345").is_none());
    }

    #[test]
    fn test_cleanup_stale_tmp_dirs_keeps_fresh() {
        run_with_temp_home(|_home| {
            let dir = get_companions_dir();
            std::fs::create_dir_all(&dir).unwrap();
            // 新鲜 tmp（当前毫秒）应保留。
            let fresh = dir.join(new_tmp_dir_name("companion-fresh123456"));
            std::fs::create_dir_all(&fresh).unwrap();
            // 过期 tmp（时间戳为 0 = 1970）应清理。
            let stale = dir.join(format!("{TMP_PREFIX}companion-stale12345-0-0"));
            std::fs::create_dir_all(&stale).unwrap();

            cleanup_stale_tmp_dirs();

            assert!(fresh.exists(), "新鲜 tmp 应保留");
            assert!(!stale.exists(), "过期 tmp 应清理");
        });
    }

    #[test]
    fn test_concurrent_import_same_source_yields_single_companion() {
        run_with_temp_home(|home| {
            let src = home.join("同款并发");
            make_valid_model(&src, "m.model3.json");

            let mut handles = Vec::new();
            for _ in 0..4 {
                let src = src.clone();
                handles.push(std::thread::spawn(move || {
                    import_from_dir(&src).map(|(_m, already)| already)
                }));
            }
            let results: Vec<bool> = handles
                .into_iter()
                .map(|h| h.join().unwrap().unwrap())
                .collect();
            // 至少一个不是 already_imported；其余是。
            assert!(results.iter().any(|a| !a));
            assert_eq!(results.iter().filter(|a| !*a).count(), 1);

            let lib = load_library_fast().unwrap();
            assert_eq!(lib.models.len(), 1);
            // 磁盘上只有一个正式托管目录。
            let dirs: Vec<_> = std::fs::read_dir(get_companions_dir())
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.starts_with("companion-"))
                .collect();
            assert_eq!(dirs.len(), 1, "并发导入只能有一个正式目录: {dirs:?}");
        });
    }
}
