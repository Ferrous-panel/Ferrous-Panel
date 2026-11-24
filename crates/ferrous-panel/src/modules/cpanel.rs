use crate::api::contract::{
    BackupArchiveItem, CpanelData, CronJobItem, DnsRecordItem, SftpAccountItem,
};
use crate::config::Config;
use crate::error::PanelError;
use crate::modules::{CronRunMeta, ModuleStore};
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::{CronSchedule, Domain, SafeLinuxUser, SqlIdentifier};
use crate::webserver::{atomic_write_file, CommandExecutor};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::io::Read as _;
use std::path::{Component, Path};
use std::time::Duration;
use subtle::ConstantTimeEq;

const CRON_TIMEOUT: Duration = Duration::from_secs(30);
const BACKUP_TIMEOUT: Duration = Duration::from_secs(120);
const CRON_META_PREFIX: &str = "# ferrous-cron:";
const CRON_DISABLED_PREFIX: &str = "#DISABLED# ";

/// Parses and validates a shell-free cron command string into `(AllowedBinary, Vec<String>)`.
pub fn parse_allowlisted_cron_command(
    raw: &str,
) -> Result<(AllowedBinary, Vec<String>, String), PanelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return Err(PanelError::Validation(
            "cron command must be 1..=512 characters".to_string(),
        ));
    }
    if trimmed.contains('\0')
        || trimmed.contains('\n')
        || trimmed.contains('\r')
        || trimmed.contains(';')
        || trimmed.contains("&&")
        || trimmed.contains("||")
        || trimmed.contains('|')
        || trimmed.contains('`')
        || trimmed.contains("$(")
        || trimmed.contains('>')
        || trimmed.contains('<')
    {
        return Err(PanelError::Forbidden(
            "cron command contains forbidden shell/control metacharacters (sh -c is forbidden)"
                .to_string(),
        ));
    }

    let tokens: Vec<String> = trimmed.split_whitespace().map(ToOwned::to_owned).collect();
    let Some(first) = tokens.first() else {
        return Err(PanelError::Validation("cron command is empty".to_string()));
    };
    let bin = AllowedBinary::from_token(first).ok_or_else(|| {
        PanelError::Forbidden(format!(
            "cron executable '{first}' is not in the allowlisted system binary list"
        ))
    })?;

    let args = tokens[1..].to_vec();
    Ok((bin, args, trimmed.to_string()))
}

#[derive(Debug, Clone)]
struct ParsedCronEntry {
    id: String,
    title: String,
    target_app: String,
    schedule: String,
    command: String,
    enabled: bool,
}

fn sanitize_meta_field(raw: &str) -> String {
    raw.trim()
        .replace(['\n', '\r', '|', '=', '#'], " ")
        .chars()
        .take(96)
        .collect()
}

fn parse_crontab_entries(stdout: &str) -> Vec<ParsedCronEntry> {
    let mut entries = Vec::new();
    let lines: Vec<&str> = stdout.lines().collect();
    let mut i = 0usize;

    while i < lines.len() {
        let line = lines[i].trim();
        if let Some(meta_raw) = line.strip_prefix(CRON_META_PREFIX) {
            let mut id = String::new();
            let mut title = String::new();
            let mut target_app = String::new();

            for kv in meta_raw.split('|') {
                if let Some((k, v)) = kv.split_once('=') {
                    match k.trim() {
                        "id" => id = v.trim().to_string(),
                        "title" => title = v.trim().to_string(),
                        "app" => target_app = v.trim().to_string(),
                        _ => {}
                    }
                }
            }

            if let Some(next_raw) = lines.get(i + 1).map(|s| s.trim()) {
                let (enabled, cron_line) =
                    if let Some(rest) = next_raw.strip_prefix(CRON_DISABLED_PREFIX) {
                        (false, rest.trim())
                    } else if let Some(rest) = next_raw.strip_prefix("# disabled ") {
                        (false, rest.trim())
                    } else {
                        (true, next_raw)
                    };

                let parts: Vec<&str> = cron_line.splitn(6, char::is_whitespace).collect();
                if parts.len() == 6 && !id.is_empty() {
                    let schedule = parts[0..5].join(" ");
                    let command = parts[5].trim().to_string();
                    entries.push(ParsedCronEntry {
                        id,
                        title: if title.is_empty() {
                            command.clone()
                        } else {
                            title
                        },
                        target_app,
                        schedule,
                        command,
                        enabled,
                    });
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }

    entries
}

fn render_crontab_entries(entries: &[ParsedCronEntry], unmanaged_lines: &[String]) -> String {
    let mut out = String::new();
    for line in unmanaged_lines {
        out.push_str(line);
        out.push('\n');
    }
    for entry in entries {
        out.push_str(&format!(
            "{CRON_META_PREFIX}id={}|title={}|app={}\n",
            entry.id,
            sanitize_meta_field(&entry.title),
            sanitize_meta_field(&entry.target_app)
        ));
        if entry.enabled {
            out.push_str(&format!("{} {}\n", entry.schedule, entry.command));
        } else {
            out.push_str(&format!(
                "{CRON_DISABLED_PREFIX}{} {}\n",
                entry.schedule, entry.command
            ));
        }
    }
    out
}

fn extract_unmanaged_lines(stdout: &str) -> Vec<String> {
    let mut unmanaged = Vec::new();
    let lines: Vec<&str> = stdout.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with(CRON_META_PREFIX) && i + 1 < lines.len() {
            i += 2;
            continue;
        }
        if !trimmed.is_empty() {
            unmanaged.push(lines[i].to_string());
        }
        i += 1;
    }
    unmanaged
}

async fn read_crontab_raw(
    executor: &dyn CommandExecutor,
    user: &SafeLinuxUser,
) -> Result<String, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Crontab) {
        return Err(PanelError::Conflict(
            "crontab binary is not installed on this host".to_string(),
        ));
    }
    let u = user.as_str();
    let out = executor
        .run(AllowedBinary::Crontab, &["-u", u, "-l"], CRON_TIMEOUT)
        .await?;
    if !out.success {
        let low = out.stderr.to_ascii_lowercase();
        if low.contains("no crontab") {
            return Ok(String::new());
        }
        return Err(PanelError::CommandFailed {
            binary: "crontab",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out.stdout)
}

async fn write_crontab_raw(
    executor: &dyn CommandExecutor,
    user: &SafeLinuxUser,
    content: &str,
) -> Result<(), PanelError> {
    let tmp_path = std::env::temp_dir().join(format!(
        "ferrous-crontab-{}-{}.cron",
        user.as_str(),
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    atomic_write_file(&tmp_path, content.as_bytes(), 0o600)?;
    let tmp_str = tmp_path.display().to_string();
    let out = executor
        .run(
            AllowedBinary::Crontab,
            &["-u", user.as_str(), &tmp_str],
            CRON_TIMEOUT,
        )
        .await;
    let _ = std::fs::remove_file(&tmp_path);
    let res = out?;
    if !res.success {
        return Err(PanelError::CommandFailed {
            binary: "crontab",
            exit_code: res.exit_code,
            stderr: res.stderr,
        });
    }
    Ok(())
}

async fn list_cron_jobs_for_default_users(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
) -> Result<Vec<CronJobItem>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Crontab) {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for user_str in ["www-data", "root", "postgres"] {
        let Ok(user) = SafeLinuxUser::parse(user_str) else {
            continue;
        };
        let Ok(raw) = read_crontab_raw(executor, &user).await else {
            continue;
        };
        for entry in parse_crontab_entries(&raw) {
            let meta = store.get_cron_run(&entry.id)?.unwrap_or(CronRunMeta {
                id: entry.id.clone(),
                last_run_at: String::new(),
                last_exit_code: 0,
                last_output: String::new(),
            });
            items.push(CronJobItem {
                id: entry.id,
                title: entry.title,
                schedule: entry.schedule,
                command: entry.command,
                user: user.as_str().to_string(),
                target_app: entry.target_app,
                enabled: entry.enabled,
                last_run_at: meta.last_run_at,
                last_exit_code: meta.last_exit_code,
                last_output: meta.last_output,
            });
        }
    }
    Ok(items)
}

pub async fn add_cron_job(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    title: &str,
    schedule_raw: &str,
    command_raw: &str,
    user_raw: &str,
    target_app_raw: &str,
) -> Result<Vec<CronJobItem>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Crontab) {
        return Err(PanelError::Conflict(
            "crontab binary is not installed on this host".to_string(),
        ));
    }
    let schedule = CronSchedule::parse(schedule_raw)?;
    let user = SafeLinuxUser::parse(if user_raw.trim().is_empty() {
        "www-data"
    } else {
        user_raw
    })?;
    let (_bin, _args, clean_command) = parse_allowlisted_cron_command(command_raw)?;
    let target_app = if target_app_raw.trim().is_empty() || target_app_raw.trim() == "system" {
        "system".to_string()
    } else {
        Domain::parse(target_app_raw)?.as_str().to_string()
    };
    let clean_title = sanitize_meta_field(title);
    if clean_title.is_empty() {
        return Err(PanelError::Validation(
            "cron title cannot be empty".to_string(),
        ));
    }

    let raw = read_crontab_raw(executor, &user).await?;
    let unmanaged = extract_unmanaged_lines(&raw);
    let mut entries = parse_crontab_entries(&raw);
    let id = format!(
        "cron-{}",
        Utc::now().timestamp_millis()
    );
    entries.push(ParsedCronEntry {
        id,
        title: clean_title,
        target_app,
        schedule: schedule.as_str().to_string(),
        command: clean_command,
        enabled: true,
    });
    let next_raw = render_crontab_entries(&entries, &unmanaged);
    write_crontab_raw(executor, &user, &next_raw).await?;
    list_cron_jobs_for_default_users(store, executor).await
}

pub async fn toggle_cron_job(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<CronJobItem>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Crontab) {
        return Err(PanelError::Conflict(
            "crontab binary is not installed on this host".to_string(),
        ));
    }
    for user_str in ["www-data", "root", "postgres"] {
        let user = SafeLinuxUser::parse(user_str)?;
        let Ok(raw) = read_crontab_raw(executor, &user).await else {
            continue;
        };
        let unmanaged = extract_unmanaged_lines(&raw);
        let mut entries = parse_crontab_entries(&raw);
        if let Some(target) = entries.iter_mut().find(|e| e.id == id) {
            target.enabled = !target.enabled;
            let next_raw = render_crontab_entries(&entries, &unmanaged);
            write_crontab_raw(executor, &user, &next_raw).await?;
            return list_cron_jobs_for_default_users(store, executor).await;
        }
    }
    Err(PanelError::NotFound(format!("cron job '{id}' not found")))
}

pub async fn delete_cron_job(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<CronJobItem>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Crontab) {
        return Err(PanelError::Conflict(
            "crontab binary is not installed on this host".to_string(),
        ));
    }
    for user_str in ["www-data", "root", "postgres"] {
        let user = SafeLinuxUser::parse(user_str)?;
        let Ok(raw) = read_crontab_raw(executor, &user).await else {
            continue;
        };
        let unmanaged = extract_unmanaged_lines(&raw);
        let mut entries = parse_crontab_entries(&raw);
        let before = entries.len();
        entries.retain(|e| e.id != id);
        if entries.len() != before {
            let next_raw = render_crontab_entries(&entries, &unmanaged);
            write_crontab_raw(executor, &user, &next_raw).await?;
            store.delete_cron_run(id)?;
            return list_cron_jobs_for_default_users(store, executor).await;
        }
    }
    Err(PanelError::NotFound(format!("cron job '{id}' not found")))
}

pub async fn run_cron_job_now(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<CronJobItem>, PanelError> {
    let jobs = list_cron_jobs_for_default_users(store, executor).await?;
    let Some(job) = jobs.into_iter().find(|j| j.id == id) else {
        return Err(PanelError::NotFound(format!("cron job '{id}' not found")));
    };
    let (bin, args, _) = parse_allowlisted_cron_command(&job.command)?;
    if !executor.is_binary_available(bin) {
        return Err(PanelError::Conflict(format!(
            "binary '{}' required for cron job is not installed",
            bin.label()
        )));
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = executor.run(bin, &arg_refs, CRON_TIMEOUT).await?;
    let combined = if out.stderr.is_empty() {
        out.stdout
    } else if out.stdout.is_empty() {
        out.stderr
    } else {
        format!("{}\n{}", out.stdout, out.stderr)
    };
    store.upsert_cron_run(&CronRunMeta {
        id: id.to_string(),
        last_run_at: Utc::now().to_rfc3339(),
        last_exit_code: out.exit_code.unwrap_or(-1),
        last_output: combined.chars().take(512).collect(),
    })?;
    list_cron_jobs_for_default_users(store, executor).await
}

fn sha256_file_hex(path: &Path) -> Result<String, PanelError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

fn validate_tar_zst_listing(stdout: &str) -> Result<(), PanelError> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('l') || trimmed.starts_with('h') {
            return Err(PanelError::Forbidden(format!(
                "backup archive contains forbidden symlink/hardlink entry: {trimmed}"
            )));
        }
        if let Some(entry_path) = trimmed.split_whitespace().last() {
            if entry_path.starts_with('/') || entry_path.contains('\\') || entry_path.contains('\0')
            {
                return Err(PanelError::Forbidden(format!(
                    "backup archive entry escapes web root: {entry_path}"
                )));
            }
            for comp in Path::new(entry_path).components() {
                if matches!(
                    comp,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                ) {
                    return Err(PanelError::Forbidden(format!(
                        "backup archive entry contains forbidden traversal: {entry_path}"
                    )));
                }
            }
        }
    }
    Ok(())
}

pub fn list_backups_verified(store: &ModuleStore, config: &Config) -> Result<Vec<BackupArchiveItem>, PanelError> {
    let raw = store.list_backups()?;
    let mut out = Vec::with_capacity(raw.len());
    for mut item in raw {
        if let Ok(safe) = SafePath::under_root(&config.backups_dir, &item.filename) {
            if let Ok(meta) = std::fs::metadata(safe.as_path()) {
                item.size_mb = ((meta.len() as f64 / 1_048_576.0) * 100.0).round() / 100.0;
                item.status = "verified".to_string();
            } else {
                item.status = "missing_on_disk".to_string();
            }
        } else {
            item.status = "invalid_path".to_string();
        }
        out.push(item);
    }
    Ok(out)
}

pub async fn create_backup(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    target_app_raw: &str,
    database_included_raw: &str,
) -> Result<Vec<BackupArchiveItem>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Tar) {
        return Err(PanelError::Conflict(
            "tar binary is not installed on this host".to_string(),
        ));
    }
    if config.is_production() && !executor.is_binary_available(AllowedBinary::Zstd) {
        return Err(PanelError::Conflict(
            "zstd binary is not installed on this host".to_string(),
        ));
    }

    let domain = Domain::parse(target_app_raw)?;
    let apps = store.list_apps()?;
    let app_dir = if let Some(app) = apps
        .into_iter()
        .find(|a| a.domain.eq_ignore_ascii_case(domain.as_str()))
    {
        SafePath::under_root(&config.wwwroot, &app.working_dir)?
    } else {
        SafePath::under_root(&config.wwwroot, domain.as_str())?
    };
    if !app_dir.as_path().is_dir() {
        return Err(PanelError::NotFound(format!(
            "website working directory '{}' does not exist",
            app_dir.as_path().display()
        )));
    }

    std::fs::create_dir_all(&config.backups_dir)?;
    let stamp = Utc::now().format("%Y%m%d_%H%M%S");
    let safe_domain_slug = domain.as_str().replace('.', "_");
    let filename = format!("{safe_domain_slug}_{stamp}.tar.zst");
    let archive_path = SafePath::under_root(&config.backups_dir, &filename)?;
    let archive_str = archive_path.as_path().display().to_string();

    let db_label = database_included_raw.trim();
    let include_db = !db_label.is_empty() && !db_label.contains("Không đính kèm");
    let staging_dir = std::env::temp_dir().join(format!(
        "ferrous-bk-db-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));

    let mut dump_added = false;
    if include_db {
        // Accept formats like `"PostgreSQL 17 · commerce_prod"` or `"commerce_prod"`.
        let db_name_raw = db_label
            .rsplit('·')
            .next()
            .unwrap_or(db_label)
            .trim();
        let db_ident = SqlIdentifier::parse(db_name_raw)?;
        let dbs = store.list_databases()?;
        let db_meta = dbs
            .into_iter()
            .find(|d| d.name == db_ident.as_str())
            .ok_or_else(|| {
                PanelError::NotFound(format!("database '{}' not registered", db_ident.as_str()))
            })?;
        std::fs::create_dir_all(&staging_dir)?;
        let dump_file = staging_dir.join("database.sql");
        let dump_str = dump_file.display().to_string();

        if db_meta.engine.to_ascii_lowercase().contains("sqlite") {
            let sql_dump =
                crate::modules::databases::export_database_sql(store, executor, &db_meta.id)
                    .await?;
            std::fs::write(&dump_file, sql_dump.as_bytes())?;
        } else if db_meta.engine.to_ascii_lowercase().contains("postgres") {
            if !executor.is_binary_available(AllowedBinary::PgDump) {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Err(PanelError::Conflict(
                    "pg_dump is not installed on this host".to_string(),
                ));
            }
            let out = executor
                .run(
                    AllowedBinary::PgDump,
                    &["--clean", "--if-exists", "-f", &dump_str, db_ident.as_str()],
                    BACKUP_TIMEOUT,
                )
                .await?;
            if !out.success {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Err(PanelError::CommandFailed {
                    binary: "pg_dump",
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
        } else {
            if !executor.is_binary_available(AllowedBinary::Mysqldump) {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Err(PanelError::Conflict(
                    "mysqldump is not installed on this host".to_string(),
                ));
            }
            let r_arg = format!("--result-file={dump_str}");
            let out = executor
                .run(
                    AllowedBinary::Mysqldump,
                    &["--single-transaction", &r_arg, db_ident.as_str()],
                    BACKUP_TIMEOUT,
                )
                .await?;
            if !out.success {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Err(PanelError::CommandFailed {
                    binary: "mysqldump",
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
        }
        dump_added = true;
    }

    let app_dir_str = app_dir.as_path().display().to_string();
    let tar_out = if dump_added {
        let stage_str = staging_dir.display().to_string();
        executor
            .run(
                AllowedBinary::Tar,
                &[
                    "--zstd",
                    "-cf",
                    &archive_str,
                    "-C",
                    &app_dir_str,
                    ".",
                    "-C",
                    &stage_str,
                    "database.sql",
                ],
                BACKUP_TIMEOUT,
            )
            .await
    } else {
        executor
            .run(
                AllowedBinary::Tar,
                &["--zstd", "-cf", &archive_str, "-C", &app_dir_str, "."],
                BACKUP_TIMEOUT,
            )
            .await
    };
    if dump_added {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }

    let tar_res = tar_out?;
    if !tar_res.success {
        return Err(PanelError::CommandFailed {
            binary: "tar",
            exit_code: tar_res.exit_code,
            stderr: tar_res.stderr,
        });
    }

    let size_bytes = std::fs::metadata(archive_path.as_path())?.len();
    let size_mb = ((size_bytes as f64 / 1_048_576.0) * 100.0).round() / 100.0;
    let sha256 = sha256_file_hex(archive_path.as_path())?;

    let item = BackupArchiveItem {
        id: format!("bk-{}", Utc::now().timestamp_millis()),
        filename,
        target_app: domain.as_str().to_string(),
        database_included: if dump_added {
            db_label.to_string()
        } else {
            "Không đính kèm Database".to_string()
        },
        size_mb,
        compression: "zstd".to_string(),
        sha256,
        created_at: Utc::now().to_rfc3339(),
        status: "verified".to_string(),
    };
    store.insert_backup(&item)?;
    list_backups_verified(store, config)
}

pub async fn restore_backup(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<BackupArchiveItem>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Tar) {
        return Err(PanelError::Conflict(
            "tar binary is not installed on this host".to_string(),
        ));
    }
    let item = store
        .get_backup(id)?
        .ok_or_else(|| PanelError::NotFound(format!("backup '{id}' not found")))?;
    let archive_path = SafePath::under_root(&config.backups_dir, &item.filename)?;
    if !archive_path.as_path().is_file() {
        return Err(PanelError::NotFound(format!(
            "backup archive '{}' missing on disk",
            item.filename
        )));
    }

    // Verify real SHA-256 digest before extracting.
    let actual_sha = sha256_file_hex(archive_path.as_path())?;
    let digest_ok: bool = actual_sha.as_bytes().ct_eq(item.sha256.as_bytes()).into();
    if !digest_ok {
        return Err(PanelError::Forbidden(
            "backup archive SHA-256 integrity verification failed; restore aborted".to_string(),
        ));
    }

    let archive_str = archive_path.as_path().display().to_string();
    let inspect = executor
        .run(
            AllowedBinary::Tar,
            &["--zstd", "-tvf", &archive_str],
            BACKUP_TIMEOUT,
        )
        .await?;
    if !inspect.success {
        return Err(PanelError::CommandFailed {
            binary: "tar",
            exit_code: inspect.exit_code,
            stderr: inspect.stderr,
        });
    }
    validate_tar_zst_listing(&inspect.stdout)?;

    let domain = Domain::parse(&item.target_app)?;
    let apps = store.list_apps()?;
    let target_dir = if let Some(app) = apps
        .into_iter()
        .find(|a| a.domain.eq_ignore_ascii_case(domain.as_str()))
    {
        SafePath::under_root(&config.wwwroot, &app.working_dir)?
    } else {
        SafePath::under_root(&config.wwwroot, domain.as_str())?
    };
    std::fs::create_dir_all(target_dir.as_path())?;
    let target_str = target_dir.as_path().display().to_string();

    let extract = executor
        .run(
            AllowedBinary::Tar,
            &[
                "--zstd",
                "--no-same-owner",
                "--no-same-permissions",
                "-xf",
                &archive_str,
                "-C",
                &target_str,
            ],
            BACKUP_TIMEOUT,
        )
        .await?;
    if !extract.success {
        return Err(PanelError::CommandFailed {
            binary: "tar",
            exit_code: extract.exit_code,
            stderr: extract.stderr,
        });
    }

    list_backups_verified(store, config)
}

pub fn delete_backup(
    store: &ModuleStore,
    config: &Config,
    id: &str,
) -> Result<Vec<BackupArchiveItem>, PanelError> {
    let item = store
        .get_backup(id)?
        .ok_or_else(|| PanelError::NotFound(format!("backup '{id}' not found")))?;
    let archive_path = SafePath::under_root(&config.backups_dir, &item.filename)?;
    if archive_path.as_path().is_file() {
        std::fs::remove_file(archive_path.as_path())?;
    }
    store.delete_backup(id)?;
    list_backups_verified(store, config)
}

const KV_DNS_RECORDS: &str = "cpanel.dns_records";
const KV_SFTP_ACCOUNTS: &str = "cpanel.sftp_accounts";

async fn write_dns_zone_files(
    config: &Config,
    executor: &dyn CommandExecutor,
    records: &[DnsRecordItem],
) -> Result<(), PanelError> {
    let zones_dir = config.data_dir.join("dns/zones");
    std::fs::create_dir_all(&zones_dir)?;
    let mut by_domain: std::collections::BTreeMap<String, Vec<&DnsRecordItem>> =
        std::collections::BTreeMap::new();
    for rec in records {
        by_domain.entry(rec.domain.clone()).or_default().push(rec);
    }
    let serial = Utc::now().format("%Y%m%d%H").to_string();
    let bind_zones_dir = Path::new("/etc/bind/zones");
    let sync_bind = config.is_production() && Path::new("/etc/bind").is_dir();

    for (domain, list) in by_domain {
        let mut buf = format!(
            "; RFC-1035 DNS Zone File for {domain} (Managed by FerrousPanel)\n\
             $ORIGIN {domain}.\n\
             $TTL 300\n\
             @ IN SOA ns1.{domain}. hostmaster.{domain}. ( {serial} 3600 900 1209600 300 )\n\
             @ IN NS ns1.{domain}.\n"
        );
        for r in list {
            let prio = r.priority.map(|p| format!("{p} ")).unwrap_or_default();
            let rendered_val = if r.record_type == "TXT"
                && !(r.value.starts_with('"') && r.value.ends_with('"'))
            {
                format!("\"{}\"", r.value.replace('"', "\\\""))
            } else {
                r.value.clone()
            };
            buf.push_str(&format!(
                "{} {} IN {} {}{}\n",
                r.host, r.ttl, r.record_type, prio, rendered_val
            ));
        }
        let zone_path = zones_dir.join(format!("{domain}.zone"));
        crate::webserver::atomic_write_file(&zone_path, buf.as_bytes(), 0o644)?;

        if sync_bind {
            let _ = std::fs::create_dir_all(bind_zones_dir);
            let bind_zone_path = bind_zones_dir.join(format!("db.{domain}"));
            let prev = std::fs::read(&bind_zone_path).ok();
            crate::webserver::atomic_write_file(&bind_zone_path, buf.as_bytes(), 0o644)?;
            let zone_str = bind_zone_path.display().to_string();

            if executor.is_binary_available(AllowedBinary::NamedCheckzone) {
                let chk = executor
                    .run(
                        AllowedBinary::NamedCheckzone,
                        &[&domain, &zone_str],
                        CRON_TIMEOUT,
                    )
                    .await?;
                if !chk.success {
                    if let Some(ref bytes) = prev {
                        let _ = crate::webserver::atomic_write_file(&bind_zone_path, bytes, 0o644);
                    } else {
                        let _ = std::fs::remove_file(&bind_zone_path);
                    }
                    return Err(PanelError::Validation(format!(
                        "BIND9 named-checkzone rejected zone '{domain}' (rolled back): {}",
                        if chk.stderr.trim().is_empty() { chk.stdout } else { chk.stderr }
                    )));
                }
            }
            if executor.is_binary_available(AllowedBinary::Rndc) {
                let _ = executor
                    .run(AllowedBinary::Rndc, &["reload", &domain], CRON_TIMEOUT)
                    .await;
            }
        }
    }
    Ok(())
}

pub fn list_dns_records(store: &ModuleStore) -> Result<Vec<DnsRecordItem>, PanelError> {
    Ok(store.get_json::<Vec<DnsRecordItem>>(KV_DNS_RECORDS)?.unwrap_or_default())
}

#[allow(clippy::too_many_arguments)]
pub async fn add_dns_record(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    domain: &str,
    record_type: &str,
    host: &str,
    value: &str,
    ttl: u32,
    priority: Option<u16>,
) -> Result<Vec<DnsRecordItem>, PanelError> {
    let clean_domain = Domain::parse(domain)?;
    let rtype = record_type.trim().to_ascii_uppercase();
    if !matches!(rtype.as_str(), "A" | "AAAA" | "CNAME" | "MX" | "TXT" | "CAA") {
        return Err(PanelError::Validation(format!(
            "unsupported DNS record type '{record_type}'"
        )));
    }
    let clean_host = host.trim();
    let clean_val = value.trim();
    if clean_host.is_empty() || clean_val.is_empty() {
        return Err(PanelError::Validation(
            "DNS host and value cannot be empty".to_string(),
        ));
    }
    if clean_host.contains('\n') || clean_val.contains('\n') {
        return Err(PanelError::Validation(
            "DNS record fields cannot contain newlines".to_string(),
        ));
    }
    let bounded_ttl = ttl.clamp(60, 604_800);
    let mut records = list_dns_records(store)?;
    let id = format!("dns-{:x}-{}", Utc::now().timestamp_millis(), records.len() + 1);
    records.insert(
        0,
        DnsRecordItem {
            id,
            domain: clean_domain.as_str().to_string(),
            record_type: rtype.clone(),
            host: clean_host.to_string(),
            value: clean_val.to_string(),
            ttl: bounded_ttl,
            priority: if rtype == "MX" {
                Some(priority.unwrap_or(10))
            } else {
                priority
            },
        },
    );
    write_dns_zone_files(config, executor, &records).await?;
    store.put_json(KV_DNS_RECORDS, &records)?;
    Ok(records)
}

pub async fn delete_dns_record(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<DnsRecordItem>, PanelError> {
    let mut records = list_dns_records(store)?;
    let before = records.len();
    records.retain(|r| r.id != id);
    if records.len() == before {
        return Err(PanelError::NotFound(format!("DNS record '{id}' not found")));
    }
    write_dns_zone_files(config, executor, &records).await?;
    store.put_json(KV_DNS_RECORDS, &records)?;
    Ok(records)
}

fn dir_used_mb(path: &Path) -> f64 {
    fn sum_bytes(p: &Path, depth: u8) -> u64 {
        if depth > 6 {
            return 0;
        }
        let Ok(rd) = std::fs::read_dir(p) else {
            return 0;
        };
        let mut total = 0u64;
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_file() {
                total = total.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
            } else if ft.is_dir() {
                total = total.saturating_add(sum_bytes(&entry.path(), depth + 1));
            }
        }
        total
    }
    let bytes = sum_bytes(path, 0);
    ((bytes as f64) / (1024.0 * 1024.0) * 100.0).round() / 100.0
}

pub fn list_sftp_accounts(
    store: &ModuleStore,
    config: &Config,
) -> Result<Vec<SftpAccountItem>, PanelError> {
    let mut accounts = store
        .get_json::<Vec<SftpAccountItem>>(KV_SFTP_ACCOUNTS)?
        .unwrap_or_default();
    for acc in &mut accounts {
        if let Ok(safe) = SafePath::under_root(&config.wwwroot, &acc.chroot_dir) {
            acc.used_mb = dir_used_mb(safe.as_path());
        }
    }
    Ok(accounts)
}

async fn sync_sftp_sshd_dropin(
    config: &Config,
    accounts: &[SftpAccountItem],
    executor: &dyn CommandExecutor,
) -> Result<(), PanelError> {
    let mut conf = String::from("# Managed by FerrousPanel — SFTP Chroot Jail Configuration\n");
    for acc in accounts {
        if SafeLinuxUser::parse(&acc.username).is_err() {
            continue;
        }
        conf.push_str(&format!(
            "\nMatch User {}\n    ChrootDirectory {}\n    ForceCommand internal-sftp\n    AllowTcpForwarding no\n    X11Forwarding no\n    PermitTunnel no\n",
            acc.username, acc.chroot_dir
        ));
    }

    let local_dir = config.data_dir.join("sftp");
    std::fs::create_dir_all(&local_dir)?;
    crate::webserver::atomic_write_file(
        &local_dir.join("90-ferrous-sftp.conf"),
        conf.as_bytes(),
        0o644,
    )?;

    if config.is_production() {
        if !executor.is_binary_available(AllowedBinary::Sshd) {
            return Err(PanelError::Conflict(
                "sshd binary is not installed on this host (install openssh-server)".to_string(),
            ));
        }
        let dropin_dir = Path::new("/etc/ssh/sshd_config.d");
        std::fs::create_dir_all(dropin_dir)?;
        let dropin_path = dropin_dir.join("90-ferrous-sftp.conf");
        let prev = std::fs::read(&dropin_path).ok();
        crate::webserver::atomic_write_file(&dropin_path, conf.as_bytes(), 0o644)?;

        let check = executor
            .run(AllowedBinary::Sshd, &["-t"], CRON_TIMEOUT)
            .await?;
        if !check.success {
            if let Some(old_bytes) = prev {
                let _ = crate::webserver::atomic_write_file(&dropin_path, &old_bytes, 0o644);
            } else {
                let _ = std::fs::remove_file(&dropin_path);
            }
            return Err(PanelError::Validation(format!(
                "sshd -t rejected SFTP chroot drop-in configuration (rolled back): {}",
                if check.stderr.is_empty() {
                    check.stdout
                } else {
                    check.stderr
                }
            )));
        }

        if !executor.is_binary_available(AllowedBinary::Systemctl) {
            return Err(PanelError::Conflict(
                "systemctl is not available to reload sshd".to_string(),
            ));
        }
        let reload = executor
            .run(AllowedBinary::Systemctl, &["reload", "ssh.service"], CRON_TIMEOUT)
            .await?;
        if !reload.success {
            let reload2 = executor
                .run(AllowedBinary::Systemctl, &["reload", "sshd.service"], CRON_TIMEOUT)
                .await?;
            if !reload2.success {
                if let Some(old_bytes) = prev {
                    let _ = crate::webserver::atomic_write_file(&dropin_path, &old_bytes, 0o644);
                } else {
                    let _ = std::fs::remove_file(&dropin_path);
                }
                return Err(PanelError::CommandFailed {
                    binary: "systemctl",
                    exit_code: reload2.exit_code,
                    stderr: if reload2.stderr.is_empty() {
                        reload.stderr
                    } else {
                        reload2.stderr
                    },
                });
            }
        }
    }
    Ok(())
}

pub async fn add_sftp_account(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    username: &str,
    target_app: &str,
    auth_method: &str,
    quota_mb: u64,
) -> Result<Vec<SftpAccountItem>, PanelError> {
    let clean_user = SafeLinuxUser::parse(username)?;
    let target_trimmed = target_app.trim();
    let rel_target = if target_trimmed.is_empty() {
        clean_user.as_str()
    } else {
        target_trimmed
    };
    let safe_chroot = SafePath::under_root(&config.wwwroot, rel_target)?;
    std::fs::create_dir_all(safe_chroot.as_path())?;

    let mut accounts = list_sftp_accounts(store, config)?;
    if accounts
        .iter()
        .any(|a| a.username.eq_ignore_ascii_case(clean_user.as_str()))
    {
        return Err(PanelError::Conflict(format!(
            "SFTP account '{}' already exists",
            clean_user.as_str()
        )));
    }

    let chroot_str = safe_chroot.as_path().display().to_string();
    if config.is_production() {
        if !executor.is_binary_available(AllowedBinary::Useradd) {
            return Err(PanelError::Conflict(
                "useradd binary is not available on this host".to_string(),
            ));
        }
        let safe_user =
            crate::system::cmd::validate_no_flag_injection(clean_user.as_str(), "sftp username")?;
        let exists = executor.is_binary_available(AllowedBinary::Id)
            && executor
                .run(AllowedBinary::Id, &["-u", "--", safe_user], CRON_TIMEOUT)
                .await
                .is_ok_and(|o| o.success);
        if !exists {
            let add_out = executor
                .run(
                    AllowedBinary::Useradd,
                    &[
                        "--system",
                        "--no-create-home",
                        "--home-dir",
                        &chroot_str,
                        "--shell",
                        "/usr/sbin/nologin",
                        "--",
                        safe_user,
                    ],
                    CRON_TIMEOUT,
                )
                .await?;
            if !add_out.success {
                return Err(PanelError::CommandFailed {
                    binary: "useradd",
                    exit_code: add_out.exit_code,
                    stderr: if add_out.stderr.is_empty() {
                        add_out.stdout
                    } else {
                        add_out.stderr
                    },
                });
            }
        }
    }

    let id = format!("sftp-{}", clean_user.as_str());
    let used_mb = dir_used_mb(safe_chroot.as_path());
    accounts.insert(
        0,
        SftpAccountItem {
            id,
            username: clean_user.as_str().to_string(),
            chroot_dir: chroot_str,
            target_app: rel_target.to_string(),
            auth_method: if auth_method.trim().is_empty() {
                "Ed25519 SSH Key".to_string()
            } else {
                auth_method.trim().to_string()
            },
            quota_mb: quota_mb.max(64),
            used_mb,
            status: "active".to_string(),
        },
    );
    sync_sftp_sshd_dropin(config, &accounts, executor).await?;
    store.put_json(KV_SFTP_ACCOUNTS, &accounts)?;
    Ok(accounts)
}

pub async fn delete_sftp_account(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<SftpAccountItem>, PanelError> {
    let mut accounts = list_sftp_accounts(store, config)?;
    let removed_user = accounts
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.username.clone());
    let before = accounts.len();
    accounts.retain(|a| a.id != id);
    if accounts.len() == before {
        return Err(PanelError::NotFound(format!(
            "SFTP account '{id}' not found"
        )));
    }
    sync_sftp_sshd_dropin(config, &accounts, executor).await?;
    if config.is_production() && executor.is_binary_available(AllowedBinary::Userdel) {
        if let Some(ref uname) = removed_user {
            if let Ok(valid_u) = SafeLinuxUser::parse(uname) {
                if !matches!(valid_u.as_str(), "root" | "www-data" | "ubuntu" | "admin") {
                    let del_out = executor
                        .run(AllowedBinary::Userdel, &["--", valid_u.as_str()], CRON_TIMEOUT)
                        .await?;
                    if !del_out.success && !del_out.stderr.contains("does not exist") {
                        return Err(PanelError::CommandFailed {
                            binary: "userdel",
                            exit_code: del_out.exit_code,
                            stderr: if del_out.stderr.is_empty() {
                                del_out.stdout
                            } else {
                                del_out.stderr
                            },
                        });
                    }
                }
            }
        }
    }
    store.put_json(KV_SFTP_ACCOUNTS, &accounts)?;
    Ok(accounts)
}

/// Loads cPanel overview: real cron jobs, DNS zone records, zstd backups, and SFTP chroot accounts.
pub async fn load_cpanel_overview(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<CpanelData, PanelError> {
    let cron_jobs = list_cron_jobs_for_default_users(store, executor).await?;
    let backups = list_backups_verified(store, config)?;
    let dns_records = list_dns_records(store)?;
    let sftp_accounts = list_sftp_accounts(store, config)?;

    Ok(CpanelData {
        cron_jobs,
        dns_records,
        backups,
        sftp_accounts,
    })
}
