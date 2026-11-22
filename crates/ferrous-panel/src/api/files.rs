use crate::auth::middleware::AuthenticatedUser;
use crate::auth::rate_limit::extract_client_ip;
use crate::error::PanelError;
use crate::modules::files::{self as files_mod, UploadZipRequest};
use crate::state::AppState;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNodeRequest {
    pub name: String,
    #[serde(default = "default_root_dir")]
    pub parent_dir: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub permissions: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFileRequest {
    pub content: String,
    #[serde(default)]
    pub permissions: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameNodeRequest {
    pub new_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractArchiveRequest {
    #[serde(default = "default_root_dir")]
    pub target_dir: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressDirRequest {
    #[serde(default = "default_root_dir")]
    pub source_dir: String,
    pub archive_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDownloadRequest {
    pub url: String,
    #[serde(default = "default_root_dir")]
    pub parent_dir: String,
    pub file_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadRawQuery {
    #[serde(default = "default_root_dir")]
    pub parent_dir: String,
    pub file_name: String,
}

fn default_root_dir() -> String {
    "/".to_string()
}

fn actor_and_ip(req: &HttpRequest, state: &AppState) -> (String, String) {
    let actor = req
        .extensions()
        .get::<AuthenticatedUser>()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let ip = extract_client_ip(req, &state.config);
    (actor, ip)
}

/// `GET /api/files/{app_id}` — lists real filesystem tree inside `SafePath` under `/www/wwwroot`.
pub async fn list_files(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let st = state.clone();
    let nodes = web::block(move || files_mod::list_files(&app_id, &st.modules, &st.config))
        .await
        .map_err(|_| PanelError::Internal("file listing worker failed".to_string()))??;
    Ok(HttpResponse::Ok().json(nodes))
}

/// `POST /api/files/{app_id}` — creates a file or directory inside the website root.
pub async fn create_node(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<CreateNodeRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let body = payload.into_inner();
    let audit_type = body.node_type.clone();
    let audit_name = body.name.clone();
    let audit_app = app_id.clone();
    let st = state.clone();
    let nodes = web::block(move || {
        files_mod::create_node(
            &app_id,
            &body.name,
            &body.parent_dir,
            &body.node_type,
            body.permissions.as_deref(),
            body.content.as_deref(),
            &st.modules,
            &st.config,
        )
    })
    .await
    .map_err(|_| PanelError::Internal("file creation worker failed".to_string()))??;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.create",
        &format!("appId={audit_app} type={audit_type} name={audit_name}"),
    )?;
    Ok(HttpResponse::Ok().json(nodes))
}

/// `POST /api/files/{app_id}/upload-zip` — validates entries against zip-slip and extracts/packs archive.
pub async fn upload_zip(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<UploadZipRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let body = payload.into_inner();
    let archive_name = body.archive_name.clone();
    let res = files_mod::upload_zip(
        &app_id,
        body,
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.upload_zip",
        &format!("appId={app_id} archive={archive_name}"),
    )?;
    Ok(HttpResponse::Ok().json(res))
}

/// `POST /api/files/{app_id}/upload-raw?parentDir=/&fileName=archive.zip` — uploads raw binary file up to 64 MiB.
pub async fn upload_raw(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<UploadRawQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let q = query.into_inner();
    let byte_len = body.len();
    let audit_app = app_id.clone();
    let audit_parent = q.parent_dir.clone();
    let audit_name = q.file_name.clone();
    let st = state.clone();
    let nodes = web::block(move || {
        files_mod::upload_raw_file(
            &app_id,
            &q.parent_dir,
            &q.file_name,
            &body,
            &st.modules,
            &st.config,
        )
    })
    .await
    .map_err(|_| PanelError::Internal("file upload worker failed".to_string()))??;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.upload_raw",
        &format!(
            "appId={audit_app} parentDir={audit_parent} fileName={audit_name} bytes={byte_len}"
        ),
    )?;
    Ok(HttpResponse::Ok().json(nodes))
}

/// `POST /api/files/{app_id}/compress` — compresses a subdirectory into a `.zip` archive.
pub async fn compress_dir(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<CompressDirRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let body = payload.into_inner();
    let res = files_mod::compress_dir(
        &app_id,
        &body.source_dir,
        &body.archive_name,
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.compress",
        &format!("appId={app_id} archive={}", res.archive_name),
    )?;
    Ok(HttpResponse::Ok().json(res))
}

/// `POST /api/files/{app_id}/remote-download` — SSRF-guarded `curl` download into website root.
pub async fn remote_download(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<RemoteDownloadRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let body = payload.into_inner();
    let nodes = files_mod::remote_download(
        &app_id,
        &body.url,
        &body.parent_dir,
        &body.file_name,
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.remote_download",
        &format!("appId={app_id} file={}", body.file_name),
    )?;
    Ok(HttpResponse::Ok().json(nodes))
}

/// `POST /api/files/{app_id}/batch-chmod` — applies `0755` to directories and `0644` to files.
pub async fn batch_chmod(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let app_id = path.into_inner();
    let audit_app = app_id.clone();
    let st = state.clone();
    let nodes = web::block(move || files_mod::batch_chmod(&app_id, &st.modules, &st.config))
        .await
        .map_err(|_| PanelError::Internal("batch chmod worker failed".to_string()))??;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.batch_chmod",
        &format!("appId={audit_app}"),
    )?;
    Ok(HttpResponse::Ok().json(nodes))
}

/// `POST /api/files/{app_id}/{file_id}/extract` — pre-flight anti-zip-slip inspected archive extraction.
pub async fn extract_archive(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    payload: web::Json<ExtractArchiveRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (app_id, file_id) = path.into_inner();
    let target_dir = payload.into_inner().target_dir;
    let res = files_mod::extract_archive(
        &app_id,
        &file_id,
        &target_dir,
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.extract",
        &format!("appId={app_id} archive={}", res.archive_name),
    )?;
    Ok(HttpResponse::Ok().json(res))
}

/// `POST /api/files/{app_id}/{file_id}/rename` — renames a node safely within the website root.
pub async fn rename_node(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    payload: web::Json<RenameNodeRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (app_id, file_id) = path.into_inner();
    let new_name = payload.into_inner().new_name;
    let audit_app = app_id.clone();
    let audit_name = new_name.clone();
    let st = state.clone();
    let nodes = web::block(move || {
        files_mod::rename_node(&app_id, &file_id, &new_name, &st.modules, &st.config)
    })
    .await
    .map_err(|_| PanelError::Internal("file rename worker failed".to_string()))??;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.rename",
        &format!("appId={audit_app} newName={audit_name}"),
    )?;
    Ok(HttpResponse::Ok().json(nodes))
}

/// `PUT /api/files/{app_id}/{file_id}` — atomically writes edited file content and mode.
pub async fn update_file(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    payload: web::Json<UpdateFileRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (app_id, file_id) = path.into_inner();
    let body = payload.into_inner();
    let audit_app = app_id.clone();
    let audit_file = file_id.clone();
    let st = state.clone();
    let nodes = web::block(move || {
        files_mod::update_file(
            &app_id,
            &file_id,
            &body.content,
            body.permissions.as_deref(),
            &st.modules,
            &st.config,
        )
    })
    .await
    .map_err(|_| PanelError::Internal("file update worker failed".to_string()))??;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.update",
        &format!("appId={audit_app} fileId={audit_file}"),
    )?;
    Ok(HttpResponse::Ok().json(nodes))
}

/// `DELETE /api/files/{app_id}/{file_id}` — deletes a file or subdirectory inside the website root.
pub async fn delete_node(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (app_id, file_id) = path.into_inner();
    let audit_app = app_id.clone();
    let audit_file = file_id.clone();
    let st = state.clone();
    let nodes = web::block(move || files_mod::delete_node(&app_id, &file_id, &st.modules, &st.config))
        .await
        .map_err(|_| PanelError::Internal("file delete worker failed".to_string()))??;

    let (actor, ip) = actor_and_ip(&req, &state);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "files.delete",
        &format!("appId={audit_app} fileId={audit_file}"),
    )?;
    Ok(HttpResponse::Ok().json(nodes))
}
