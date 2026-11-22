use crate::error::PanelError;
use crate::modules::{apps as apps_mod, cpanel as cpanel_mod, PanelJobRecord};
use crate::state::AppState;
use actix_web::web;
use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};

static JOB_SEQ: AtomicU64 = AtomicU64::new(1);

fn generate_job_id(kind: &str) -> String {
    let seq = JOB_SEQ.fetch_add(1, Ordering::Relaxed);
    let ts = Utc::now().timestamp_millis();
    let clean_kind: String = kind
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(12)
        .collect();
    format!("job-{clean_kind}-{ts}-{seq}")
}

fn new_queued_job(kind: &str, title: String, initial_log: String) -> PanelJobRecord {
    let now = Utc::now().to_rfc3339();
    PanelJobRecord {
        id: generate_job_id(kind),
        kind: kind.to_string(),
        title,
        status: "queued".to_string(),
        progress: 5,
        output: initial_log,
        error: None,
        created_at: now.clone(),
        updated_at: now,
        finished_at: None,
    }
}

/// Spawns a non-blocking background rebuild job for `app_id`, preventing HTTP/Cloudflare timeouts.
pub fn spawn_rebuild_app_job(
    state: web::Data<AppState>,
    app_id: String,
    actor: String,
    ip: String,
) -> Result<PanelJobRecord, PanelError> {
    let app = state
        .modules
        .get_app(&app_id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{app_id}' not found")))?;

    let job = new_queued_job(
        "app-rebuild",
        format!("Rebuild & Restart {}", app.domain),
        format!(
            "[{}] Queued rebuild for {} ({:?} {})\n",
            Utc::now().to_rfc3339(),
            app.domain,
            app.framework,
            app.runtime_version
        ),
    );
    state.modules.insert_job(&job)?;

    let job_id = job.id.clone();
    let domain = app.domain.clone();
    actix_web::rt::spawn(async move {
        let _ = state.modules.update_job(
            &job_id,
            "running",
            25,
            Some(&format!("Running build pipeline and systemd restart for {domain}...")),
            None,
            false,
        );

        match apps_mod::app_action(
            &app_id,
            "rebuild",
            &state.modules,
            &state.config,
            &state.webserver,
        )
        .await
        {
            Ok(updated) => {
                let _ = state.modules.update_job(
                    &job_id,
                    "completed",
                    100,
                    Some(&format!(
                        "Rebuild completed for {} (status={:?}, commit={})",
                        updated.domain, updated.status, updated.last_deploy_commit
                    )),
                    None,
                    true,
                );
                let _ = state.auth_store.append_audit_log(
                    Some(&actor),
                    &ip,
                    "job.app.rebuild.completed",
                    &format!("jobId={job_id} appId={app_id} domain={}", updated.domain),
                );
            }
            Err(err) => {
                let msg = err.to_string();
                let _ = state.modules.update_job(
                    &job_id,
                    "failed",
                    100,
                    Some(&format!("Rebuild failed: {msg}")),
                    Some(&msg),
                    true,
                );
                let _ = state.auth_store.append_audit_log(
                    Some(&actor),
                    &ip,
                    "job.app.rebuild.failed",
                    &format!("jobId={job_id} appId={app_id} err={msg}"),
                );
            }
        }
    });

    Ok(job)
}

/// Spawns a non-blocking background Git pull/build/restart job for `app_id`.
pub fn spawn_git_deploy_job(
    state: web::Data<AppState>,
    app_id: String,
    actor: String,
    ip: String,
) -> Result<PanelJobRecord, PanelError> {
    let app = state
        .modules
        .get_app(&app_id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{app_id}' not found")))?;

    let job = new_queued_job(
        "git-deploy",
        format!("Git Deploy {}", app.domain),
        format!(
            "[{}] Queued Git deploy for {} (branch={})\n",
            Utc::now().to_rfc3339(),
            app.domain,
            app.git_branch
        ),
    );
    state.modules.insert_job(&job)?;

    let job_id = job.id.clone();
    let domain = app.domain.clone();
    actix_web::rt::spawn(async move {
        let _ = state.modules.update_job(
            &job_id,
            "running",
            30,
            Some(&format!("Pulling latest revision and redeploying {domain}...")),
            None,
            false,
        );

        match apps_mod::git_deploy_app(
            &app_id,
            &state.modules,
            &state.config,
            &state.webserver,
        )
        .await
        {
            Ok(updated) => {
                let _ = state.modules.update_job(
                    &job_id,
                    "completed",
                    100,
                    Some(&format!(
                        "Git deploy succeeded for {} at commit {}",
                        updated.domain, updated.last_deploy_commit
                    )),
                    None,
                    true,
                );
                let _ = state.auth_store.append_audit_log(
                    Some(&actor),
                    &ip,
                    "job.app.git_deploy.completed",
                    &format!(
                        "jobId={job_id} appId={app_id} domain={} commit={}",
                        updated.domain, updated.last_deploy_commit
                    ),
                );
            }
            Err(err) => {
                let msg = err.to_string();
                let _ = state.modules.update_job(
                    &job_id,
                    "failed",
                    100,
                    Some(&format!("Git deploy failed: {msg}")),
                    Some(&msg),
                    true,
                );
                let _ = state.auth_store.append_audit_log(
                    Some(&actor),
                    &ip,
                    "job.app.git_deploy.failed",
                    &format!("jobId={job_id} appId={app_id} err={msg}"),
                );
            }
        }
    });

    Ok(job)
}

/// Spawns a non-blocking Certbot SSL issuance/renewal job for `app_id`.
pub fn spawn_renew_ssl_job(
    state: web::Data<AppState>,
    app_id: String,
    actor: String,
    ip: String,
) -> Result<PanelJobRecord, PanelError> {
    let app = state
        .modules
        .get_app(&app_id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{app_id}' not found")))?;

    let job = new_queued_job(
        "ssl-renew",
        format!("Renew TLS Certificate {}", app.domain),
        format!(
            "[{}] Queued Certbot TLS renewal for {}\n",
            Utc::now().to_rfc3339(),
            app.domain
        ),
    );
    state.modules.insert_job(&job)?;

    let job_id = job.id.clone();
    let domain = app.domain.clone();
    actix_web::rt::spawn(async move {
        let _ = state.modules.update_job(
            &job_id,
            "running",
            35,
            Some(&format!("Requesting/renewing TLS certificate for {domain}...")),
            None,
            false,
        );

        match apps_mod::renew_ssl(
            &app_id,
            &state.modules,
            &state.config,
            &state.webserver,
        )
        .await
        {
            Ok(updated) => {
                let _ = state.modules.update_job(
                    &job_id,
                    "completed",
                    100,
                    Some(&format!(
                        "TLS certificate active for {} (sslEnabled={})",
                        updated.domain, updated.ssl_enabled
                    )),
                    None,
                    true,
                );
                let _ = state.auth_store.append_audit_log(
                    Some(&actor),
                    &ip,
                    "job.app.ssl_renew.completed",
                    &format!("jobId={job_id} appId={app_id} domain={}", updated.domain),
                );
            }
            Err(err) => {
                let msg = err.to_string();
                let _ = state.modules.update_job(
                    &job_id,
                    "failed",
                    100,
                    Some(&format!("TLS renewal failed: {msg}")),
                    Some(&msg),
                    true,
                );
                let _ = state.auth_store.append_audit_log(
                    Some(&actor),
                    &ip,
                    "job.app.ssl_renew.failed",
                    &format!("jobId={job_id} appId={app_id} err={msg}"),
                );
            }
        }
    });

    Ok(job)
}

/// Spawns a non-blocking Zstd website + database backup archive job.
pub fn spawn_create_backup_job(
    state: web::Data<AppState>,
    target_app: String,
    database_included: String,
    actor: String,
    ip: String,
) -> Result<PanelJobRecord, PanelError> {
    let clean_target = target_app.trim().to_string();
    if clean_target.is_empty() {
        return Err(PanelError::Validation(
            "targetApp is required to queue a backup job".to_string(),
        ));
    }

    let job = new_queued_job(
        "cpanel-backup",
        format!("Zstd Backup {clean_target}"),
        format!(
            "[{}] Queued Zstd backup archive for targetApp={} db={}\n",
            Utc::now().to_rfc3339(),
            clean_target,
            database_included
        ),
    );
    state.modules.insert_job(&job)?;

    let job_id = job.id.clone();
    actix_web::rt::spawn(async move {
        let _ = state.modules.update_job(
            &job_id,
            "running",
            30,
            Some(&format!(
                "Archiving {clean_target} (database={database_included}) with tar --zstd..."
            )),
            None,
            false,
        );

        match cpanel_mod::create_backup(
            &state.modules,
            &state.config,
            state.webserver.executor().as_ref(),
            &clean_target,
            &database_included,
        )
        .await
        {
            Ok(backups) => {
                let latest_desc = backups
                    .first()
                    .map(|b| format!("{} ({:.2} MiB, sha256={})", b.filename, b.size_mb, b.sha256))
                    .unwrap_or_else(|| "archive created".to_string());
                let _ = state.modules.update_job(
                    &job_id,
                    "completed",
                    100,
                    Some(&format!("Backup completed: {latest_desc}")),
                    None,
                    true,
                );
                let _ = state.auth_store.append_audit_log(
                    Some(&actor),
                    &ip,
                    "job.cpanel.backup.completed",
                    &format!(
                        "jobId={job_id} targetApp={clean_target} db={database_included}"
                    ),
                );
            }
            Err(err) => {
                let msg = err.to_string();
                let _ = state.modules.update_job(
                    &job_id,
                    "failed",
                    100,
                    Some(&format!("Backup failed: {msg}")),
                    Some(&msg),
                    true,
                );
                let _ = state.auth_store.append_audit_log(
                    Some(&actor),
                    &ip,
                    "job.cpanel.backup.failed",
                    &format!("jobId={job_id} targetApp={clean_target} err={msg}"),
                );
            }
        }
    });

    Ok(job)
}
