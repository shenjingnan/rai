//! 模型资产清单解析与下载安装。
//!
//! 模型元数据编译期嵌入（`include_str!`），运行时从用户目录
//! `~/.zapmomo/models/<name>` 安装/查找，供 CLI（`kws install-model`）与
//! GUI（下载按钮）复用。流程与 `scripts/download-kws-model.sh` 一致：
//! 下载 → sha256 校验 → 临时目录解压 → 原子落位，幂等可重跑。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::config::settings::get_models_dir;
use crate::kws::config::{
    DEFAULT_DECODER, DEFAULT_ENCODER, DEFAULT_JOINER, DEFAULT_KEYWORDS_REL, DEFAULT_TOKENS,
};

/// `models/manifest.json` 的顶层结构。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelManifest {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    pub assets: Vec<ModelAsset>,
}

/// 单个模型资产。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelAsset {
    pub name: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub version: String,
    pub archive: String,
    pub source: String,
    pub sha256: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub license: String,
}

/// 编译期嵌入的清单 JSON（随仓库入库，打包后不依赖外部文件）。
const MANIFEST_JSON: &str = include_str!("../../models/manifest.json");

/// 解析一次并缓存。
fn manifest() -> &'static ModelManifest {
    static CACHE: OnceLock<ModelManifest> = OnceLock::new();
    CACHE.get_or_init(|| serde_json::from_str(MANIFEST_JSON).expect("内嵌模型清单无效"))
}

/// 默认唤醒词模型资产（清单中第一个 `role == "wake-word"` 的资产，找不到则取首个）。
pub fn default_asset() -> &'static ModelAsset {
    manifest()
        .assets
        .iter()
        .find(|a| a.role == "wake-word")
        .or_else(|| manifest().assets.first())
        .expect("模型清单为空")
}

/// 用户模型根目录：`~/.zapmomo/models`
pub fn user_models_dir() -> PathBuf {
    get_models_dir()
}

/// 默认模型安装目录：`~/.zapmomo/models/<name>`
pub fn user_model_dir() -> PathBuf {
    get_models_dir().join(&default_asset().name)
}

/// 下载/安装阶段（CLI 打日志 / GUI 推事件共用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStage {
    Downloading,
    Verifying,
    Extracting,
    Done,
}

/// 下载进度回调载荷。
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub stage: DownloadStage,
    /// 下载阶段 0..=100；其它阶段为 `-1`（不确定进度）。
    pub percent: f64,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub message: String,
}

pub type ProgressFn<'a> = dyn FnMut(DownloadProgress) + Send + 'a;

/// 模型安装错误。
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),
    #[error("下载失败（重试后仍失败）: {0}")]
    Download(String),
    #[error("sha256 校验失败（期望 {expected}，实际 {actual}）")]
    Sha256Mismatch { expected: String, actual: String },
    #[error("解压失败: {0}")]
    Extract(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

/// 目标目录是否已装好模型（5 个核心文件齐全）。
pub fn is_installed(dest_dir: &Path) -> bool {
    [
        DEFAULT_ENCODER,
        DEFAULT_DECODER,
        DEFAULT_JOINER,
        DEFAULT_TOKENS,
        DEFAULT_KEYWORDS_REL,
    ]
    .iter()
    .all(|f| dest_dir.join(f).is_file())
}

/// 安装默认唤醒词模型到 `dest_dir`（默认 `~/.zapmomo/models/<name>`）。
///
/// 幂等：已安装且 `force` 为假时直接返回。下载过程中回调进度。
pub fn install_model_to(
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    install_asset_to(default_asset(), dest_dir, force, on_progress)
}

/// 按指定资产安装（测试/多模型可复用）。
fn install_asset_to(
    asset: &ModelAsset,
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    let parent = dest_dir
        .parent()
        .ok_or_else(|| ModelError::Extract("目标目录缺少父目录".to_string()))?;

    if !force && is_installed(dest_dir) {
        on_progress(progress(DownloadStage::Done, 100.0, dest_dir, "模型已安装"));
        return Ok(());
    }

    std::fs::create_dir_all(parent)?;
    let tmp_archive = parent.join(format!(".{}.tmp", asset.archive));

    download_to(&asset.source, &tmp_archive, asset.size_bytes, on_progress)?;

    on_progress(progress(
        DownloadStage::Verifying,
        -1.0,
        dest_dir,
        "校验 sha256",
    ));
    verify_sha256(&tmp_archive, &asset.sha256)?;

    on_progress(progress(
        DownloadStage::Extracting,
        -1.0,
        dest_dir,
        "解压中",
    ));
    extract_and_place(&tmp_archive, dest_dir)?;

    on_progress(progress(
        DownloadStage::Done,
        100.0,
        dest_dir,
        "模型安装完成",
    ));
    Ok(())
}

fn progress(
    stage: DownloadStage,
    percent: f64,
    _dest_dir: &Path,
    message: &str,
) -> DownloadProgress {
    DownloadProgress {
        stage,
        percent,
        bytes_downloaded: 0,
        total_bytes: 0,
        message: message.to_string(),
    }
}

/// 流式下载到临时文件，带进度回调；失败重试 3 次（退避等待）。
fn download_to(
    url: &str,
    tmp_archive: &Path,
    manifest_total: u64,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    let mut last_err: Option<ModelError> = None;
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(400 * (1 << attempt)));
        }
        match try_download_once(url, tmp_archive, manifest_total, on_progress) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.map_or_else(
        || ModelError::Download("未知错误".to_string()),
        |e| ModelError::Download(e.to_string()),
    ))
}

fn try_download_once(
    url: &str,
    tmp_archive: &Path,
    manifest_total: u64,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| ModelError::Http(e.to_string()))?;
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(manifest_total);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(tmp_archive)?;
    let mut buf = [0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        let percent = if total > 0 {
            ((done as f64 / total as f64) * 100.0).min(100.0)
        } else {
            -1.0
        };
        on_progress(DownloadProgress {
            stage: DownloadStage::Downloading,
            percent,
            bytes_downloaded: done,
            total_bytes: total,
            message: format!("下载中 {:.1}%", percent.max(0.0)),
        });
    }
    file.flush()?;
    Ok(())
}

/// 对临时压缩包整包计算 sha256 并比对；不匹配则删除损坏文件并报错。
fn verify_sha256(path: &Path, expected: &str) -> Result<(), ModelError> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        let _ = std::fs::remove_file(path);
        return Err(ModelError::Sha256Mismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

/// 解压 tar.bz2 到同父目录临时目录，再把顶层模型目录原子移到目标位置。
fn extract_and_place(tmp_archive: &Path, dest_dir: &Path) -> Result<(), ModelError> {
    let parent = dest_dir
        .parent()
        .ok_or_else(|| ModelError::Extract("目标目录缺少父目录".to_string()))?;
    let name = dest_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let tmp_extract = parent.join(format!(".{name}.extract"));
    std::fs::create_dir_all(&tmp_extract)?;

    let file = std::fs::File::open(tmp_archive)?;
    let bz = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(bz);
    archive
        .unpack(&tmp_extract)
        .map_err(|e| ModelError::Extract(e.to_string()))?;

    // 定位顶层模型目录：优先 <name>，否则退化为唯一的顶层项（兼容不同包内布局）。
    let src = tmp_extract.join(&name);
    let src = if src.is_dir() {
        src
    } else {
        let mut entries = std::fs::read_dir(&tmp_extract)?.filter_map(Result::ok);
        let top = entries
            .next()
            .map(|e| e.path())
            .ok_or_else(|| ModelError::Extract("压缩包内容为空".to_string()))?;
        if entries.next().is_some() {
            return Err(ModelError::Extract(
                "压缩包顶层存在多个目录，无法确定模型根目录".to_string(),
            ));
        }
        top
    };

    // 原子落位：目标已存在先移除（Windows 上 rename 覆盖目录会失败）。
    if dest_dir.exists() {
        std::fs::remove_dir_all(dest_dir)?;
    }
    std::fs::rename(&src, dest_dir)?;
    std::fs::remove_dir_all(&tmp_extract)?;
    let _ = std::fs::remove_file(tmp_archive);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn mini_tarbz2(prefix: &str) -> Vec<u8> {
        use bzip2::Compression;
        use bzip2::write::BzEncoder;
        let mut bz = BzEncoder::new(Vec::new(), Compression::default());
        {
            let mut ar = tar::Builder::new(&mut bz);
            let base = format!("{prefix}/");
            let mut dir = tar::Header::new_gnu();
            dir.set_entry_type(tar::EntryType::Directory);
            dir.set_size(0);
            dir.set_mode(0o755);
            dir.set_username("test").unwrap();
            dir.set_groupname("test").unwrap();
            dir.set_cksum();
            ar.append_data(&mut dir, &base, std::io::empty()).unwrap();

            let mut f = |rel: &str, bytes: &[u8]| {
                let mut h = tar::Header::new_gnu();
                h.set_size(bytes.len() as u64);
                h.set_mode(0o644);
                h.set_username("test").unwrap();
                h.set_groupname("test").unwrap();
                h.set_cksum();
                ar.append_data(&mut h, format!("{base}{rel}"), bytes)
                    .unwrap();
            };
            f(DEFAULT_ENCODER, b"enc-onnx-bytes");
            f(DEFAULT_DECODER, b"dec-onnx-bytes");
            f(DEFAULT_JOINER, b"joiner-onnx-bytes");
            f(DEFAULT_TOKENS, b"token symbols");
            f(DEFAULT_KEYWORDS_REL, b"k w @KW\n");
            ar.finish().unwrap();
        }
        bz.finish().unwrap()
    }

    /// 起一个本地 HTTP 服务，每个连接都返回给定字节，返回请求 URL。
    fn serve_many(bytes: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let payload = std::sync::Arc::new(bytes);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut sock) = stream else { break };
                let payload = payload.clone();
                std::thread::spawn(move || {
                    let mut buf = [0u8; 1024];
                    let _ = sock.read(&mut buf);
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    let _ = sock.write_all(head.as_bytes());
                    let _ = sock.write_all(&payload);
                });
            }
        });
        format!("http://{addr}/model.tar.bz2")
    }

    fn asset_for(source: &str, sha256: &str, archive: &str) -> ModelAsset {
        ModelAsset {
            name: "test-kws-model".to_string(),
            role: "wake-word".to_string(),
            version: "test".to_string(),
            archive: archive.to_string(),
            source: source.to_string(),
            sha256: sha256.to_string(),
            size_bytes: 0,
            license: "Apache-2.0".to_string(),
        }
    }

    fn sha256_hex(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(data))
    }

    #[test]
    fn test_manifest_default_asset() {
        let a = default_asset();
        assert!(!a.name.is_empty());
        assert!(a.source.starts_with("http"));
        assert_eq!(a.sha256.len(), 64);
        // 与仓库 models/ 目录名一致（单一事实来源）
        assert!(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("models")
                .join(&a.name)
                .is_dir()
        );
    }

    #[test]
    fn test_verify_sha256_ok_and_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("blob");
        std::fs::write(&p, b"hello").unwrap();
        assert!(verify_sha256(&p, &sha256_hex(b"hello")).is_ok());
        // 错误校验值：报错且删除损坏文件
        let p2 = dir.path().join("bad");
        std::fs::write(&p2, b"hello").unwrap();
        let err = verify_sha256(&p2, &"0".repeat(64)).unwrap_err();
        assert!(matches!(err, ModelError::Sha256Mismatch { .. }));
        assert!(!p2.exists());
    }

    #[test]
    fn test_extract_and_place_mini_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("mini.tar.bz2");
        std::fs::write(&archive, mini_tarbz2("test-kws-model")).unwrap();
        let dest = dir.path().join("test-kws-model");
        extract_and_place(&archive, &dest).unwrap();
        assert!(is_installed(&dest));
        assert!(!archive.exists());
        assert!(!dir.path().join(".test-kws-model.extract").exists());
    }

    #[test]
    fn test_install_full_flow_via_local_server() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = mini_tarbz2("test-kws-model");
        let url = serve_many(bytes.clone());
        let archive = "mini.tar.bz2".to_string();
        let asset = asset_for(&url, &sha256_hex(&bytes), &archive);

        let dest = dir.path().join("test-kws-model");
        let mut stages = Vec::new();
        install_asset_to(&asset, &dest, false, &mut |p| stages.push(p.stage)).unwrap();
        assert!(is_installed(&dest));

        let expected = [
            DownloadStage::Downloading,
            DownloadStage::Verifying,
            DownloadStage::Extracting,
            DownloadStage::Done,
        ];
        assert_eq!(stages, expected);
    }

    #[test]
    fn test_install_idempotent_skips_when_installed() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("test-kws-model");
        // 直接摆好核心文件，模拟已安装
        std::fs::create_dir_all(dest.join("test_wavs")).unwrap();
        std::fs::write(dest.join(DEFAULT_ENCODER), b"e").unwrap();
        std::fs::write(dest.join(DEFAULT_DECODER), b"d").unwrap();
        std::fs::write(dest.join(DEFAULT_JOINER), b"j").unwrap();
        std::fs::write(dest.join(DEFAULT_TOKENS), b"t").unwrap();
        std::fs::write(dest.join(DEFAULT_KEYWORDS_REL), b"k").unwrap();

        let mut stages = Vec::new();
        install_asset_to(&default_asset(), &dest, false, &mut |p| {
            stages.push(p.stage)
        })
        .unwrap();
        assert_eq!(stages, vec![DownloadStage::Done]);
    }

    #[test]
    fn test_install_force_reinstalls() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = mini_tarbz2("test-kws-model");
        let url = serve_many(bytes.clone());
        let asset = asset_for(&url, &sha256_hex(&bytes), "mini.tar.bz2");
        let dest = dir.path().join("test-kws-model");

        // 先装好，再 force 重装 → 应重新走完整流程
        install_asset_to(&asset, &dest, false, &mut |_| {}).unwrap();
        let mut stages = Vec::new();
        install_asset_to(&asset, &dest, true, &mut |p| stages.push(p.stage)).unwrap();
        assert!(is_installed(&dest));
        assert!(stages.contains(&DownloadStage::Downloading));
    }

    #[test]
    fn test_install_sha256_mismatch_errors() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = mini_tarbz2("test-kws-model");
        let url = serve_many(bytes);
        let asset = asset_for(&url, &"0".repeat(64), "mini.tar.bz2");
        let dest = dir.path().join("test-kws-model");
        let err = install_asset_to(&asset, &dest, false, &mut |_| {}).unwrap_err();
        assert!(matches!(err, ModelError::Sha256Mismatch { .. }));
        assert!(!dest.exists());
    }
}
