use crate::api::actor_and_ip;
use crate::api::cpanel::CreateBackupRequest;
use crate::error::PanelError;
use crate::modules::{jobs as jobs_mod, PanelJobRecord};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const MAX_LONG_POLL_MS: u64 = 25_000;
const POLL_STEP_MS: u64 = 250;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListJobsQuery {
    #[serde(default = "default_jobs_limit")]
    pub limit: usize,
}

fn default_jobs_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollJobQuery {
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_wait_ms")]
    pub wait_ms: u64,
}

fn default_wait_ms() -> u64 {
    15_000
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPollResponse {
    pub job: PanelJobRecord,
    pub log_chunk: String,
    pub next_offset: usize,
    pub done: bool,
}

fn is_terminal_status(status: &str) -> bool {
    status == "completed" || status == "failed"
}

fn slice_from_offset(output: &str, offset: usize) -> (String, usize) {
    if offset >= output.len() {
        return (String::new(), output.len());
    }
    let mut start = offset;
    while start < output.len() && !output.is_char_boundary(start) {
        start += 1;
    }
    (output[start..].to_string(), output.len())
}

/// `GET /api/jobs` — lists newest persisted background jobs.
pub async fn list_jobs(
    state: web::Data<AppState>,
    query: web::Query<ListJobsQuery>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let limit = query.into_inner().limit;
    let st = state.clone();
    let jobs = web::block(move || st.modules.list_jobs(limit))
        .await
        .map_err(|_| PanelError::Internal("job listing worker failed".to_string()))??;
    Ok(HttpResponse::Ok().json(jobs))
}

/// `GET /api/jobs/{id}` — retrieves current status and full log output for a background job.
pub async fn get_job(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let id_lookup = id.clone();
    let st = state.clone();
    let job = web::block(move || st.modules.get_job(&id_lookup))
        .await
        .map_err(|_| PanelError::Internal("job query worker failed".to_string()))??
        .ok_or_else(|| PanelError::NotFound(format!("background job '{id}' not found")))?;
    Ok(HttpResponse::Ok().json(job))
}

/// `GET /api/jobs/{id}/poll?offset=0&waitMs=15000` — Cloudflare-safe incremental long-poll for live job progress.
pub async fn poll_job(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<PollJobQuery>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let q = query.into_inner();
    let max_wait = Duration::from_millis(q.wait_ms.min(MAX_LONG_POLL_MS));
    let deadline = Instant::now() + max_wait;

    loop {
        let id_lookup = id.clone();
        let st = state.clone();
        let job = web::block(move || st.modules.get_job(&id_lookup))
            .await
            .map_err(|_| PanelError::Internal("job poll worker failed".to_string()))??
            .ok_or_else(|| PanelError::NotFound(format!("background job '{id}' not found")))?;

        let done = is_terminal_status(&job.status);
        let (log_chunk, next_offset) = slice_from_offset(&job.output, q.offset);

        if done || !log_chunk.is_empty() || Instant::now() >= deadline {
            return Ok(HttpResponse::Ok().json(JobPollResponse {
                job,
                log_chunk,
                next_offset,
                done,
            }));
        }

        tokio::time::sleep(Duration::from_millis(POLL_STEP_MS)).await;
    }
}

/// `POST /api/jobs/apps/{id}/rebuild` — queues an asynchronous rebuild job (`202 Accepted`).
pub async fn start_rebuild_app_job(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let (actor, ip) = actor_and_ip(&req);
    let job = jobs_mod::spawn_rebuild_app_job(state.clone(), app_id.clone(), actor.clone(), ip.clone())?;
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "job.app.rebuild.queued",
        &format!("jobId={} appId={app_id}", job.id),
    )?;
    Ok(HttpResponse::Accepted().json(job))
}

/// `POST /api/jobs/apps/{id}/git-deploy` — queues an asynchronous Git deploy job (`202 Accepted`).
pub async fn start_git_deploy_job(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let (actor, ip) = actor_and_ip(&req);
    let job = jobs_mod::spawn_git_deploy_job(state.clone(), app_id.clone(), actor.clone(), ip.clone())?;
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "job.app.git_deploy.queued",
        &format!("jobId={} appId={app_id}", job.id),
    )?;
    Ok(HttpResponse::Accepted().json(job))
}

/// `POST /api/jobs/apps/{id}/renew-ssl` — queues an asynchronous Certbot SSL renewal job (`202 Accepted`).
pub async fn start_renew_ssl_job(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let (actor, ip) = actor_and_ip(&req);
    let job = jobs_mod::spawn_renew_ssl_job(state.clone(), app_id.clone(), actor.clone(), ip.clone())?;
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "job.app.ssl_renew.queued",
        &format!("jobId={} appId={app_id}", job.id),
    )?;
    Ok(HttpResponse::Accepted().json(job))
}

/// `POST /api/jobs/cpanel/backups` — queues an asynchronous Zstd backup archive job (`202 Accepted`).
pub async fn start_create_backup_job(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<CreateBackupRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let (actor, ip) = actor_and_ip(&req);
    let job = jobs_mod::spawn_create_backup_job(
        state.clone(),
        body.target_app.clone(),
        body.database_included.clone(),
        actor.clone(),
        ip.clone(),
    )?;
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "job.cpanel.backup.queued",
        &format!(
            "jobId={} targetApp={} db={}",
            job.id, body.target_app, body.database_included
        ),
    )?;
    Ok(HttpResponse::Accepted().json(job))
}
