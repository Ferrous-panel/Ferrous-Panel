use super::common::*;

// ============================================================================
// 7. SSH Manager & Panel Admin
// ============================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigPayload {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub listen_address: Option<String>,
    #[serde(default)]
    pub permit_root_login: Option<String>,
    #[serde(default)]
    pub password_authentication: Option<bool>,
    #[serde(default)]
    pub pubkey_authentication: Option<bool>,
    #[serde(default)]
    pub max_auth_tries: Option<u32>,
    #[serde(default)]
    pub client_alive_interval_sec: Option<u32>,
    #[serde(default)]
    pub client_alive_count_max: Option<u32>,
    #[serde(default)]
    pub allow_tcp_forwarding: Option<bool>,
    #[serde(default)]
    pub sftp_chroot_enabled: Option<bool>,
    #[serde(default)]
    pub allow_users: Option<Vec<String>>,
    #[serde(default)]
    pub kex_algorithms: Option<String>,
    #[serde(default)]
    pub confirm_port_change: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddSshKeyPayload {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub client_hint: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSshKeyPayload {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub client_hint: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecSshPayload {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub cwd: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PanelAdminPayload {
    #[serde(default)]
    pub admin_username: Option<String>,
    #[serde(default)]
    pub admin_password: Option<String>,
    #[serde(default)]
    pub panel_port: Option<u16>,
    #[serde(default)]
    pub security_entrance: Option<String>,
    #[serde(default)]
    pub custom_domain: Option<String>,
    #[serde(default)]
    pub ssl_enabled: Option<bool>,
    #[serde(default)]
    pub two_factor_enabled: Option<bool>,
    #[serde(default)]
    pub allowed_admin_ips: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PanelCliActionPayload {
    #[serde(default)]
    pub option: u8,
    #[serde(default)]
    pub value: Option<String>,
}

/// `GET /api/ssh` — loads SSH daemon config, authorized Ed25519 keys, active sessions, and Panel Admin metadata.
pub async fn get_ssh_panel(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let data = ext::load_ssh_panel(
        &state.modules,
        &state.config,
        &state.auth_store,
        state.webserver.executor().as_ref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(data))
}

/// `PUT /api/ssh/config` — updates OpenSSH/russh daemon hardening settings.
pub async fn put_ssh_config(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<SshConfigPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let data = ext::update_ssh_config(
        &state.modules,
        &state.config,
        &state.auth_store,
        state.webserver.executor().as_ref(),
        ext::SshConfigPatch {
            port: body.port,
            listen_address: body.listen_address,
            permit_root_login: body.permit_root_login,
            password_authentication: body.password_authentication,
            pubkey_authentication: body.pubkey_authentication,
            max_auth_tries: body.max_auth_tries,
            client_alive_interval_sec: body.client_alive_interval_sec,
            client_alive_count_max: body.client_alive_count_max,
            allow_tcp_forwarding: body.allow_tcp_forwarding,
            sftp_chroot_enabled: body.sftp_chroot_enabled,
            allow_users: body.allow_users,
            kex_algorithms: body.kex_algorithms,
            confirm_port_change: body.confirm_port_change,
        },
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.config.update",
        &format!("port={}", data.config.port),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/ssh/keys` — imports an authorized SSH public key.
pub async fn post_ssh_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<AddSshKeyPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let data = ext::add_ssh_authorized_key(
        &state.modules,
        &state.config,
        &state.auth_store,
        state.webserver.executor().as_ref(),
        &body.label,
        &body.public_key,
        &body.client_hint,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.key.add",
        &format!("label={}", body.label),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/ssh/keys/generate` — generates a new Ed25519 keypair and registers the public key.
pub async fn post_ssh_key_generate(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<GenerateSshKeyPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let (generated, ssh_data) = ext::generate_ssh_keypair(
        &state.modules,
        &state.config,
        &state.auth_store,
        state.webserver.executor().as_ref(),
        &body.label,
        &body.client_hint,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.key.generate",
        &format!("label={} fingerprint={}", generated.label, generated.fingerprint),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "generated": generated,
        "sshData": ssh_data,
    })))
}

/// `DELETE /api/ssh/keys/{id}` — removes an authorized SSH key.
pub async fn delete_ssh_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::delete_ssh_key(
        &state.modules,
        &state.config,
        &state.auth_store,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state
        .auth_store
        .append_audit_log(Some(&actor), &ip, "ssh.key.delete", &format!("id={id}"))?;
    Ok(HttpResponse::Ok().json(data))
}

/// `DELETE /api/ssh/sessions/{id}` — terminates an active SSH PTY session.
pub async fn delete_ssh_session(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::terminate_ssh_session(
        &state.modules,
        &state.config,
        &state.auth_store,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.session.terminate",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/ssh/exec` — executes a safe allowlisted Web SSH command (no shell evaluation).
pub async fn post_ssh_exec(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<ExecSshPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let out = ext::exec_web_ssh_command(
        &state.config,
        state.webserver.executor().as_ref(),
        &body.command,
        &body.cwd,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.exec",
        &format!("cmd={}", body.command),
    )?;
    Ok(HttpResponse::Ok().json(out))
}

/// `GET /api/panel-admin` — returns Panel Admin account and security entrance configuration.
pub async fn get_panel_admin(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let admin = ext::load_panel_admin(&state.modules, &state.config, &state.auth_store)?;
    Ok(HttpResponse::Ok().json(admin))
}

/// `PUT /api/panel-admin` — updates Panel Admin username, password, port, or security entrance.
pub async fn put_panel_admin(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<PanelAdminPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let admin = ext::update_panel_admin(
        &state.modules,
        &state.config,
        &state.auth_store,
        state.webserver.executor().as_ref(),
        ext::PanelAdminPatch {
            admin_username: body.admin_username,
            admin_password: body.admin_password,
            panel_port: body.panel_port,
            security_entrance: body.security_entrance,
            custom_domain: body.custom_domain,
            ssl_enabled: body.ssl_enabled,
            two_factor_enabled: body.two_factor_enabled,
            allowed_admin_ips: body.allowed_admin_ips,
        },
    )
    .await?;
    if let Ok((fresh_creds, _)) = crate::auth::password::load_or_init_credentials(
        &state.config.credentials_path(),
        &state.config.admin_username,
    ) {
        if let Ok(mut guard) = state.credentials.write() {
            *guard = fresh_creds;
        }
    }

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "panel_admin.update",
        &format!("username={} port={}", admin.admin_username, admin.panel_port),
    )?;
    Ok(HttpResponse::Ok().json(admin))
}

/// `POST /api/panel-admin/cli-action` — runs an `fp` CLI menu option (1..20) from the web UI.
pub async fn post_panel_cli_action(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<PanelCliActionPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let option = if body.option == 0 { 14 } else { body.option };
    let (output, panel_admin) = ext::run_panel_cli_action(
        &state.modules,
        &state.config,
        &state.auth_store,
        &state.webserver,
        option,
        body.value.as_deref(),
    )
    .await?;
    if matches!(option, 5 | 6) {
        if let Ok((fresh_creds, _)) = crate::auth::password::load_or_init_credentials(
            &state.config.credentials_path(),
            &state.config.admin_username,
        ) {
            if let Ok(mut guard) = state.credentials.write() {
                *guard = fresh_creds;
            }
        }
    }

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "panel_admin.cli_action",
        &format!("option={option}"),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "output": output,
        "panelAdmin": panel_admin,
    })))
}

