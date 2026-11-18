use crate::api::contract::WebFileNode;
use crate::config::Config;
use crate::error::PanelError;
use crate::modules::ModuleStore;
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::OctalMode;
use crate::webserver::{atomic_write_file, CommandExecutor};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::IpAddr;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const MAX_LIST_NODES: usize = 2000;
const MAX_READ_CONTENT_BYTES: u64 = 128 * 1024;
const MAX_UPLOAD_ENTRIES: usize = 512;
const FILE_CMD_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadedZipEntry {
    pub relative_path: String,
    #[serde(default)]
    pub is_dir: bool,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadZipRequest {
    pub archive_name: String,
    #[serde(default = "default_root_slash")]
    pub parent_dir: String,
    #[serde(default)]
    pub archive_size_bytes: u64,
    #[serde(default)]
    pub extract_immediately: bool,
    #[serde(default)]
    pub entries: Vec<UploadedZipEntry>,
}

fn default_root_slash() -> String {
    "/".to_string()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipOperationResponse {
    pub files: Vec<WebFileNode>,
    pub extracted_count: usize,
    pub archive_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressResponse {
    pub files: Vec<WebFileNode>,
    pub archive_name: String,
    pub compressed_bytes: u64,
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn decode_hex_path(hex_id: &str) -> Result<String, PanelError> {
    let clean = hex_id.trim();
    if clean.is_empty() || clean.len() % 2 != 0 || clean.len() > 2048 {
        return Err(PanelError::Validation("invalid file identifier".to_string()));
    }
    let mut bytes = Vec::with_capacity(clean.len() / 2);
    let raw = clean.as_bytes();
    for pair in raw.chunks_exact(2) {
        let s = std::str::from_utf8(pair)
            .map_err(|_| PanelError::Validation("invalid file identifier encoding".to_string()))?;
        let val = u8::from_str_radix(s, 16)
            .map_err(|_| PanelError::Validation("invalid hex character in file id".to_string()))?;
        bytes.push(val);
    }
    let rel = String::from_utf8(bytes)
        .map_err(|_| PanelError::Validation("file identifier is not valid UTF-8".to_string()))?;
    if rel.contains('\0') {
        return Err(PanelError::Forbidden(
            "NUL byte in file identifier is forbidden".to_string(),
        ));
    }
    Ok(rel)
}

#[must_use]
pub fn file_id_for_relative(rel_without_leading_slash: &str) -> String {
    let clean = rel_without_leading_slash.trim_start_matches('/');
    if clean.is_empty() {
        encode_hex(b".")
    } else {
        encode_hex(clean.as_bytes())
    }
}

fn validate_single_filename(raw: &str) -> Result<String, PanelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err(PanelError::Validation(
            "file or directory name must be 1..=128 characters".to_string(),
        ));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(PanelError::Forbidden(
            "relative dot directory names '.' and '..' are forbidden".to_string(),
        ));
    }
    if trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
        || trimmed.contains("..")
    {
        return Err(PanelError::Forbidden(
            "filename contains forbidden path separator or traversal sequence".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_archive_relative_entry(raw: &str) -> Result<String, PanelError> {
    if raw.contains('\0') || raw.contains('\\') {
        return Err(PanelError::Forbidden(
            "archive entry contains forbidden backslash or NUL byte".to_string(),
        ));
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(PanelError::Validation("empty archive entry path".to_string()));
    }
    if trimmed.starts_with('/') {
        return Err(PanelError::Forbidden(format!(
            "zip-slip rejected: absolute entry path '{trimmed}'"
        )));
    }
    if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
        return Err(PanelError::Forbidden(format!(
            "zip-slip rejected: drive prefix in entry '{trimmed}'"
        )));
    }
    for comp in Path::new(trimmed).components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PanelError::Forbidden(format!(
                    "zip-slip rejected: unsafe path component in '{trimmed}'"
                )));
            }
        }
    }
    Ok(trimmed.to_string())
}

fn format_symbolic_perms(is_dir: bool, mode: u32) -> String {
    let mut s = String::with_capacity(10);
    s.push(if is_dir { 'd' } else { '-' });
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        s.push(if (bits & 0o4) != 0 { 'r' } else { '-' });
        s.push(if (bits & 0o2) != 0 { 'w' } else { '-' });
        s.push(if (bits & 0o1) != 0 { 'x' } else { '-' });
    }
    s
}

fn resolve_app_root(
    app_id: &str,
    store: &ModuleStore,
    config: &Config,
) -> Result<SafePath, PanelError> {
    let app = store
        .get_app(app_id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{app_id}' not found")))?;
    let safe = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
    std::fs::create_dir_all(safe.as_path())?;
    Ok(safe)
}

fn normalize_virtual_dir(raw: &str) -> Result<String, PanelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return Ok(String::new());
    }
    let clean = trimmed.trim_start_matches('/').trim_end_matches('/');
    if clean.contains('\0') || clean.contains('\\') {
        return Err(PanelError::Forbidden(
            "forbidden character in directory path".to_string(),
        ));
    }
    for comp in Path::new(clean).components() {
        if !matches!(comp, Component::Normal(_)) {
            return Err(PanelError::Forbidden(
                "path traversal in directory parameter is forbidden".to_string(),
            ));
        }
    }
    Ok(clean.to_string())
}

fn resolve_under_app_root(app_root: &SafePath, virtual_rel: &str) -> Result<SafePath, PanelError> {
    let clean = virtual_rel.trim().trim_start_matches('/');
    if clean.is_empty() || clean == "." {
        return Ok(app_root.clone());
    }
    SafePath::under_root(app_root.as_path(), clean)
}

pub fn list_files(
    app_id: &str,
    store: &ModuleStore,
    config: &Config,
) -> Result<Vec<WebFileNode>, PanelError> {
    let app_root = resolve_app_root(app_id, store, config)?;
    let canon_root = std::fs::canonicalize(app_root.as_path())?;

    let mut nodes = Vec::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    queue.push_back(canon_root.clone());

    while let Some(dir_path) = queue.pop_front() {
        let Ok(rd) = std::fs::read_dir(&dir_path) else {
            continue;
        };
        for entry in rd.flatten() {
            if nodes.len() >= MAX_LIST_NODES {
                break;
            }
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            // Never follow or traverse symlinks out of or inside the web root tree.
            if meta.file_type().is_symlink() {
                continue;
            }
            let Ok(rel_path) = path.strip_prefix(&canon_root) else {
                continue;
            };
            let rel_str = rel_path
                .components()
                .filter_map(|c| match c {
                    Component::Normal(s) => s.to_str(),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            if rel_str.is_empty() {
                continue;
            }

            let is_dir = meta.is_dir();
            if is_dir {
                queue.push_back(path.clone());
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let parent_dir = match rel_str.rsplit_once('/') {
                Some((parent, _)) => format!("/{parent}"),
                None => "/".to_string(),
            };
            let mode = meta.permissions().mode() & 0o777;
            let permissions = format!("{mode:04o}");
            let symbolic_perms = format_symbolic_perms(is_dir, mode);
            let owner = format!("{}:{}", meta.uid(), meta.gid());
            let modified_at = meta
                .modified()
                .ok()
                .map(|st| DateTime::<Utc>::from(st).to_rfc3339())
                .unwrap_or_else(|| Utc::now().to_rfc3339());

            let content = if !is_dir && meta.len() <= MAX_READ_CONTENT_BYTES {
                std::fs::read(&path)
                    .ok()
                    .and_then(|b| String::from_utf8(b).ok())
            } else {
                None
            };

            nodes.push(WebFileNode {
                id: file_id_for_relative(&rel_str),
                app_id: app_id.to_string(),
                name,
                path: format!("/{rel_str}"),
                parent_dir,
                node_type: if is_dir {
                    "directory".to_string()
                } else {
                    "file".to_string()
                },
                size_bytes: meta.len(),
                permissions,
                symbolic_perms,
                owner,
                modified_at,
                content,
            });
        }
    }

    Ok(nodes)
}

pub fn create_node(
    app_id: &str,
    name: &str,
    parent_dir: &str,
    node_type: &str,
    permissions: Option<&str>,
    content: Option<&str>,
    store: &ModuleStore,
    config: &Config,
) -> Result<Vec<WebFileNode>, PanelError> {
    let app_root = resolve_app_root(app_id, store, config)?;
    let clean_name = validate_single_filename(name)?;
    let clean_parent = normalize_virtual_dir(parent_dir)?;
    let rel = if clean_parent.is_empty() {
        clean_name
    } else {
        format!("{clean_parent}/{clean_name}")
    };
    let target = resolve_under_app_root(&app_root, &rel)?;
    let default_mode = if node_type == "directory" {
        "0755"
    } else {
        "0644"
    };
    let mode = OctalMode::parse(permissions.unwrap_or(default_mode))?;

    match node_type {
        "directory" => {
            std::fs::create_dir_all(target.as_path())?;
            std::fs::set_permissions(target.as_path(), std::fs::Permissions::from_mode(mode.mode()))?;
        }
        "file" => {
            atomic_write_file(
                target.as_path(),
                content.unwrap_or("").as_bytes(),
                mode.mode(),
            )?;
        }
        other => {
            return Err(PanelError::Validation(format!(
                "invalid node type '{other}'; expected 'file' or 'directory'"
            )));
        }
    }

    list_files(app_id, store, config)
}

pub fn update_file(
    app_id: &str,
    file_id: &str,
    content: &str,
    permissions: Option<&str>,
    store: &ModuleStore,
    config: &Config,
) -> Result<Vec<WebFileNode>, PanelError> {
    let app_root = resolve_app_root(app_id, store, config)?;
    let rel = decode_hex_path(file_id)?;
    let target = resolve_under_app_root(&app_root, &rel)?;

    let meta = std::fs::symlink_metadata(target.as_path())
        .map_err(|_| PanelError::NotFound("file does not exist".to_string()))?;
    if meta.file_type().is_symlink() {
        return Err(PanelError::Forbidden(
            "editing symbolic links is forbidden".to_string(),
        ));
    }
    if !meta.is_file() {
        return Err(PanelError::Validation(
            "target path is a directory, not a regular file".to_string(),
        ));
    }

    let mode = match permissions {
        Some(p) if !p.trim().is_empty() => OctalMode::parse(p)?.mode(),
        _ => meta.permissions().mode() & 0o777,
    };

    atomic_write_file(target.as_path(), content.as_bytes(), mode)?;
    list_files(app_id, store, config)
}

pub fn rename_node(
    app_id: &str,
    file_id: &str,
    new_name: &str,
    store: &ModuleStore,
    config: &Config,
) -> Result<Vec<WebFileNode>, PanelError> {
    let app_root = resolve_app_root(app_id, store, config)?;
    let rel = decode_hex_path(file_id)?;
    let src = resolve_under_app_root(&app_root, &rel)?;
    if src.as_path() == app_root.as_path() {
        return Err(PanelError::Forbidden(
            "cannot rename web root directory itself".to_string(),
        ));
    }
    if std::fs::symlink_metadata(src.as_path()).is_err() {
        return Err(PanelError::NotFound("source file or directory not found".to_string()));
    }

    let clean_new = validate_single_filename(new_name)?;
    let dst_rel = match rel.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{clean_new}"),
        None => clean_new,
    };
    let dst = resolve_under_app_root(&app_root, &dst_rel)?;
    std::fs::rename(src.as_path(), dst.as_path())?;
    list_files(app_id, store, config)
}

pub fn delete_node(
    app_id: &str,
    file_id: &str,
    store: &ModuleStore,
    config: &Config,
) -> Result<Vec<WebFileNode>, PanelError> {
    let app_root = resolve_app_root(app_id, store, config)?;
    let rel = decode_hex_path(file_id)?;
    let target = resolve_under_app_root(&app_root, &rel)?;
    if target.as_path() == app_root.as_path() {
        return Err(PanelError::Forbidden(
            "deleting website root directory via file manager is forbidden".to_string(),
        ));
    }
    let meta = std::fs::symlink_metadata(target.as_path())
        .map_err(|_| PanelError::NotFound("target path not found".to_string()))?;
    if meta.file_type().is_symlink() {
        return Err(PanelError::Forbidden(
            "symbolic links cannot be removed via file manager".to_string(),
        ));
    }
    if meta.is_dir() {
        std::fs::remove_dir_all(target.as_path())?;
    } else {
        std::fs::remove_file(target.as_path())?;
    }
    list_files(app_id, store, config)
}

pub fn batch_chmod(
    app_id: &str,
    store: &ModuleStore,
    config: &Config,
) -> Result<Vec<WebFileNode>, PanelError> {
    let app_root = resolve_app_root(app_id, store, config)?;
    let canon_root = std::fs::canonicalize(app_root.as_path())?;
    let mut stack = vec![canon_root];
    let mut count = 0usize;

    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            count += 1;
            if count > MAX_LIST_NODES {
                break;
            }
            let p = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))?;
                stack.push(p);
            } else if meta.is_file() {
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644))?;
            }
        }
    }

    list_files(app_id, store, config)
}

pub async fn upload_zip(
    app_id: &str,
    req: UploadZipRequest,
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<ZipOperationResponse, PanelError> {
    let app_root = resolve_app_root(app_id, store, config)?;
    let archive_name = validate_single_filename(&req.archive_name)?;
    let clean_parent = normalize_virtual_dir(&req.parent_dir)?;
    let parent_path = resolve_under_app_root(&app_root, &clean_parent)?;
    std::fs::create_dir_all(parent_path.as_path())?;

    if req.entries.len() > MAX_UPLOAD_ENTRIES {
        return Err(PanelError::Validation(format!(
            "archive contains more than {MAX_UPLOAD_ENTRIES} entries"
        )));
    }

    // Strict pre-flight anti-zip-slip validation BEFORE writing anything to disk.
    let mut validated_entries = Vec::with_capacity(req.entries.len());
    for entry in &req.entries {
        if entry.relative_path.trim().is_empty() {
            continue;
        }
        let rel = validate_archive_relative_entry(&entry.relative_path)?;
        let full_rel = if clean_parent.is_empty() {
            rel.clone()
        } else {
            format!("{clean_parent}/{rel}")
        };
        let resolved = resolve_under_app_root(&app_root, &full_rel)?;
        validated_entries.push((rel, resolved, entry.is_dir, entry.content.as_deref()));
    }

    let mut extracted_count = 0usize;
    if req.extract_immediately {
        for (_rel, resolved, is_dir, content) in &validated_entries {
            if *is_dir {
                std::fs::create_dir_all(resolved.as_path())?;
                std::fs::set_permissions(
                    resolved.as_path(),
                    std::fs::Permissions::from_mode(0o755),
                )?;
            } else {
                let bytes = content.unwrap_or("").as_bytes();
                atomic_write_file(resolved.as_path(), bytes, 0o644)?;
            }
            extracted_count += 1;
        }
    } else {
        // Pack staged files into a real `.zip` archive on disk using the `zip` CLI.
        if !executor.is_binary_available(AllowedBinary::Zip) {
            return Err(PanelError::Conflict(
                "zip binary is not installed; enable auto-extract or install zip".to_string(),
            ));
        }
        let staging_dir = std::env::temp_dir().join(format!(
            "ferrous-zip-stage-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&staging_dir)?;
        for (rel, _resolved, is_dir, content) in &validated_entries {
            let staged_target = SafePath::under_root(&staging_dir, rel)?;
            if *is_dir {
                std::fs::create_dir_all(staged_target.as_path())?;
            } else {
                atomic_write_file(staged_target.as_path(), content.unwrap_or("").as_bytes(), 0o644)?;
            }
        }
        let dest_rel = if clean_parent.is_empty() {
            archive_name.clone()
        } else {
            format!("{clean_parent}/{archive_name}")
        };
        let dest_archive = resolve_under_app_root(&app_root, &dest_rel)?;
        let dest_str = dest_archive.as_path().display().to_string();
        let out = executor
            .run_with_env(
                AllowedBinary::Zip,
                &["-r", "-q", &dest_str, "."],
                &[],
                Some(&staging_dir),
                FILE_CMD_TIMEOUT,
            )
            .await;
        let _ = std::fs::remove_dir_all(&staging_dir);
        let res = out?;
        if !res.success {
            return Err(PanelError::CommandFailed {
                binary: "zip",
                exit_code: res.exit_code,
                stderr: res.stderr,
            });
        }
    }

    let files = list_files(app_id, store, config)?;
    Ok(ZipOperationResponse {
        files,
        extracted_count,
        archive_name,
    })
}

fn validate_tar_listing(stdout: &str) -> Result<usize, PanelError> {
    let mut count = 0usize;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Reject symlinks ('l') and hardlinks ('h') in verbose tar listing.
        if trimmed.starts_with('l') || trimmed.starts_with('h') {
            return Err(PanelError::Forbidden(format!(
                "archive contains forbidden symlink/hardlink entry: {trimmed}"
            )));
        }
        // Last whitespace token holds entry path in `tar -tvf` or `tar -tf`.
        if let Some(entry_path) = trimmed.split_whitespace().last() {
            let _ = validate_archive_relative_entry(entry_path)?;
        }
        count += 1;
    }
    Ok(count)
}

pub async fn extract_archive(
    app_id: &str,
    file_id: &str,
    target_dir_raw: &str,
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<ZipOperationResponse, PanelError> {
    let app_root = resolve_app_root(app_id, store, config)?;
    let rel = decode_hex_path(file_id)?;
    let archive_path = resolve_under_app_root(&app_root, &rel)?;
    let archive_name = archive_path
        .as_path()
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("archive")
        .to_string();

    if !archive_path.as_path().is_file() {
        return Err(PanelError::NotFound("archive file not found".to_string()));
    }

    let clean_target = normalize_virtual_dir(target_dir_raw)?;
    let target_dir = resolve_under_app_root(&app_root, &clean_target)?;
    std::fs::create_dir_all(target_dir.as_path())?;

    let arch_str = archive_path.as_path().display().to_string();
    let target_str = target_dir.as_path().display().to_string();
    let lower = archive_name.to_ascii_lowercase();

    let extracted_count = if lower.ends_with(".zip") {
        if !executor.is_binary_available(AllowedBinary::Unzip) {
            return Err(PanelError::Conflict(
                "unzip binary is not installed on this host".to_string(),
            ));
        }
        let inspect = executor
            .run(
                AllowedBinary::Unzip,
                &["-Z1", &arch_str],
                FILE_CMD_TIMEOUT,
            )
            .await?;
        if !inspect.success {
            return Err(PanelError::CommandFailed {
                binary: "unzip",
                exit_code: inspect.exit_code,
                stderr: inspect.stderr,
            });
        }
        let mut count = 0usize;
        for line in inspect.stdout.lines() {
            let entry = line.trim();
            if entry.is_empty() {
                continue;
            }
            let _ = validate_archive_relative_entry(entry)?;
            count += 1;
        }
        let unpack = executor
            .run(
                AllowedBinary::Unzip,
                &["-o", "-q", &arch_str, "-d", &target_str],
                FILE_CMD_TIMEOUT,
            )
            .await?;
        if !unpack.success {
            return Err(PanelError::CommandFailed {
                binary: "unzip",
                exit_code: unpack.exit_code,
                stderr: unpack.stderr,
            });
        }
        count
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") || lower.ends_with(".tar.zst") {
        if !executor.is_binary_available(AllowedBinary::Tar) {
            return Err(PanelError::Conflict(
                "tar binary is not installed on this host".to_string(),
            ));
        }
        let mut inspect_args = Vec::new();
        if lower.ends_with(".tar.zst") {
            inspect_args.push("--zstd");
        } else {
            inspect_args.push("-z");
        }
        inspect_args.push("-tvf");
        inspect_args.push(&arch_str);
        let inspect = executor
            .run(AllowedBinary::Tar, &inspect_args, FILE_CMD_TIMEOUT)
            .await?;
        if !inspect.success {
            return Err(PanelError::CommandFailed {
                binary: "tar",
                exit_code: inspect.exit_code,
                stderr: inspect.stderr,
            });
        }
        let count = validate_tar_listing(&inspect.stdout)?;

        let mut unpack_args = Vec::new();
        if lower.ends_with(".tar.zst") {
            unpack_args.push("--zstd");
        } else {
            unpack_args.push("-z");
        }
        unpack_args.extend_from_slice(&[
            "--no-same-owner",
            "--no-same-permissions",
            "-xf",
            &arch_str,
            "-C",
            &target_str,
        ]);
        let unpack = executor
            .run(AllowedBinary::Tar, &unpack_args, FILE_CMD_TIMEOUT)
            .await?;
        if !unpack.success {
            return Err(PanelError::CommandFailed {
                binary: "tar",
                exit_code: unpack.exit_code,
                stderr: unpack.stderr,
            });
        }
        count
    } else {
        return Err(PanelError::Validation(
            "unsupported archive format; allowed: .zip, .tar.gz, .tgz, .tar.zst".to_string(),
        ));
    };

    let files = list_files(app_id, store, config)?;
    Ok(ZipOperationResponse {
        files,
        extracted_count,
        archive_name,
    })
}

pub async fn compress_dir(
    app_id: &str,
    source_dir_raw: &str,
    archive_name_raw: &str,
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<CompressResponse, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Zip) {
        return Err(PanelError::Conflict(
            "zip binary is not installed on this host".to_string(),
        ));
    }
    let app_root = resolve_app_root(app_id, store, config)?;
    let clean_source = normalize_virtual_dir(source_dir_raw)?;
    let source_path = resolve_under_app_root(&app_root, &clean_source)?;
    if !source_path.as_path().is_dir() {
        return Err(PanelError::NotFound("source directory does not exist".to_string()));
    }

    let archive_name = validate_single_filename(archive_name_raw)?;
    if !archive_name.to_ascii_lowercase().ends_with(".zip") {
        return Err(PanelError::Validation(
            "archiveName must end with .zip".to_string(),
        ));
    }

    let dest_rel = if clean_source.is_empty() {
        archive_name.clone()
    } else {
        format!("{clean_source}/{archive_name}")
    };
    let dest_archive = resolve_under_app_root(&app_root, &dest_rel)?;
    let dest_str = dest_archive.as_path().display().to_string();

    let out = executor
        .run_with_env(
            AllowedBinary::Zip,
            &["-r", "-q", &dest_str, "."],
            &[],
            Some(source_path.as_path()),
            FILE_CMD_TIMEOUT,
        )
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "zip",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }

    let compressed_bytes = std::fs::metadata(dest_archive.as_path())
        .map(|m| m.len())
        .unwrap_or(0);
    let files = list_files(app_id, store, config)?;
    Ok(CompressResponse {
        files,
        archive_name,
        compressed_bytes,
    })
}

fn is_forbidden_ssrf_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

pub fn validate_remote_download_url(raw: &str) -> Result<String, PanelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 {
        return Err(PanelError::Validation("URL must be 1..=2048 characters".to_string()));
    }
    if trimmed.contains('\0') || trimmed.chars().any(char::is_whitespace) {
        return Err(PanelError::Forbidden(
            "URL contains whitespace or NUL byte".to_string(),
        ));
    }
    let after_scheme = if let Some(rest) = trimmed.strip_prefix("https://") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        rest
    } else {
        return Err(PanelError::Validation(
            "only http:// and https:// URLs are allowed".to_string(),
        ));
    };

    let authority = after_scheme.split('/').next().unwrap_or("");
    if authority.contains('@') {
        return Err(PanelError::Forbidden(
            "embedded credentials in remote URL are forbidden".to_string(),
        ));
    }
    let host = if let Some(stripped) = authority.strip_prefix('[') {
        stripped.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    let lower_host = host.to_ascii_lowercase();
    if lower_host.is_empty()
        || lower_host == "localhost"
        || lower_host.ends_with(".local")
        || lower_host.ends_with(".internal")
        || lower_host == "metadata.google.internal"
    {
        return Err(PanelError::Forbidden(
            "downloading from loopback or internal hostnames is forbidden (SSRF guard)".to_string(),
        ));
    }
    if let Ok(ip) = lower_host.parse::<IpAddr>() {
        if is_forbidden_ssrf_ip(ip) {
            return Err(PanelError::Forbidden(format!(
                "downloading from non-public IP '{ip}' is forbidden (SSRF guard)"
            )));
        }
    }
    Ok(trimmed.to_string())
}

pub async fn remote_download(
    app_id: &str,
    url_raw: &str,
    parent_dir: &str,
    file_name_raw: &str,
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<Vec<WebFileNode>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Curl) {
        return Err(PanelError::Conflict(
            "curl binary is not installed on this host".to_string(),
        ));
    }
    let app_root = resolve_app_root(app_id, store, config)?;
    let url = validate_remote_download_url(url_raw)?;
    let clean_name = validate_single_filename(file_name_raw)?;
    let clean_parent = normalize_virtual_dir(parent_dir)?;
    let rel = if clean_parent.is_empty() {
        clean_name
    } else {
        format!("{clean_parent}/{clean_name}")
    };
    let dest = resolve_under_app_root(&app_root, &rel)?;
    if let Some(p) = dest.as_path().parent() {
        std::fs::create_dir_all(p)?;
    }
    let dest_str = dest.as_path().display().to_string();

    let out = executor
        .run(
            AllowedBinary::Curl,
            &[
                "-fsSL",
                "--proto",
                "=http,https",
                "--max-filesize",
                "104857600",
                "-o",
                &dest_str,
                &url,
            ],
            FILE_CMD_TIMEOUT,
        )
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "curl",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }

    list_files(app_id, store, config)
}
