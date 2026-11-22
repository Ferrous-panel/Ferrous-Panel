use super::common::*;

// ============================================================================
// 5. Mail Server (Postfix / Dovecot / Rspamd)
// ============================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddMailDomainPayload {
    #[serde(default)]
    pub domain: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateMailboxPayload {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "default_mailbox_quota")]
    pub quota_mb: u64,
}

const fn default_mailbox_quota() -> u64 {
    2048
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateForwarderPayload {
    #[serde(default)]
    pub source_email: String,
    #[serde(default)]
    pub destination_email: String,
    #[serde(default)]
    pub domain: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SendMailPayload {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body: String,
}

/// `GET /api/mail` — loads Postfix/Dovecot/Rspamd mail server state, domains, mailboxes, forwarders, and queue.
pub async fn get_mail(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let mail = ext::load_mail_server(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `POST /api/mail/domains` — adds a mail domain with Ed25519 DKIM, SPF, and DMARC selectors.
pub async fn post_mail_domain(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<AddMailDomainPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let domain = payload
        .map(|p| p.into_inner().domain)
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| "mail.ferrous-panel.local".to_string());
    let mail = ext::add_mail_domain(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &domain,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.domain.add",
        &format!("domain={domain}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `POST /api/mail/mailboxes` — creates a virtual Maildir mailbox and updates virtual maps.
pub async fn post_mail_mailbox(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<CreateMailboxPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let domain = if body.domain.trim().is_empty() {
        "ferrous-panel.local".to_string()
    } else {
        body.domain
    };
    let email = if body.email.trim().is_empty() {
        format!("postmaster@{domain}")
    } else {
        body.email
    };
    let quota = if body.quota_mb == 0 { 2048 } else { body.quota_mb };
    let mail = ext::create_mailbox(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &email,
        &domain,
        &body.display_name,
        quota,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.mailbox.create",
        &format!("email={email}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `DELETE /api/mail/mailboxes/{id}` — deletes a virtual mailbox.
pub async fn delete_mail_mailbox(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let mail = ext::delete_mailbox(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.mailbox.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `POST /api/mail/forwarders` — creates a virtual alias forwarder.
pub async fn post_mail_forwarder(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<CreateForwarderPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let domain = if body.domain.trim().is_empty() {
        "ferrous-panel.local".to_string()
    } else {
        body.domain
    };
    let src = if body.source_email.trim().is_empty() {
        format!("info@{domain}")
    } else {
        body.source_email
    };
    let dst = if body.destination_email.trim().is_empty() {
        format!("admin@{domain}")
    } else {
        body.destination_email
    };
    let mail = ext::create_forwarder(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &src,
        &dst,
        &domain,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.forwarder.create",
        &format!("src={src} dst={dst}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `DELETE /api/mail/forwarders/{id}` — deletes a virtual alias forwarder.
pub async fn delete_mail_forwarder(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let mail = ext::delete_forwarder(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.forwarder.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `POST /api/mail/send` — spools and transmits an outbound/test message via host sendmail/Postfix.
pub async fn post_mail_send(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<SendMailPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let from = if body.from.trim().is_empty() {
        "admin@ferrous-panel.local".to_string()
    } else {
        body.from
    };
    let to = if body.to.trim().is_empty() {
        "ops@ferrous-panel.local".to_string()
    } else {
        body.to
    };
    let mail = ext::send_test_mail(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &from,
        &to,
        &body.subject,
        &body.body,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.send",
        &format!("from={from} to={to}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

