use crate::state::AppState;
use actix_web::http::header;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;

const INDEX_HTML_TEMPLATE: &str = include_str!("assets/index.html");
const PANEL_CSS: &str = include_str!("assets/panel.css");
const FAVICON_SVG: &str = include_str!("assets/favicon.svg");
const APP_JS: &str = include_str!("assets/app.js");
const VIEWS_CORE_JS: &str = include_str!("assets/views-core.js");
const VIEWS_INFRA_JS: &str = include_str!("assets/views-infra.js");
const VIEWS_SECURITY_JS: &str = include_str!("assets/views-security.js");

const ASSET_CACHE_REVALIDATE: &str = "public, no-cache, must-revalidate";
const ASSET_CACHE_IMMUTABLE: &str = "public, max-age=31536000, immutable";
const HTML_CACHE_CONTROL: &str = "no-store, max-age=0";
const UI_HEADER_NAME: &str = "X-Ferrous-UI";
const UI_HEADER_VALUE: &str = "pure-rust-native";

const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
        i += 1;
    }
    hash
}

const HASH_PANEL_CSS: u64 = fnv1a64(PANEL_CSS.as_bytes());
const HASH_FAVICON_SVG: u64 = fnv1a64(FAVICON_SVG.as_bytes());
const HASH_APP_JS: u64 = fnv1a64(APP_JS.as_bytes());
const HASH_VIEWS_CORE_JS: u64 = fnv1a64(VIEWS_CORE_JS.as_bytes());
const HASH_VIEWS_INFRA_JS: u64 = fnv1a64(VIEWS_INFRA_JS.as_bytes());
const HASH_VIEWS_SECURITY_JS: u64 = fnv1a64(VIEWS_SECURITY_JS.as_bytes());

const UI_BUNDLE_HASH: u64 = HASH_PANEL_CSS
    ^ HASH_FAVICON_SVG.rotate_left(7)
    ^ HASH_APP_JS.rotate_left(13)
    ^ HASH_VIEWS_CORE_JS.rotate_left(23)
    ^ HASH_VIEWS_INFRA_JS.rotate_left(31)
    ^ HASH_VIEWS_SECURITY_JS.rotate_left(41);

#[must_use]
pub fn ui_bundle_version_tag() -> String {
    format!("{}-{:016x}", env!("CARGO_PKG_VERSION"), UI_BUNDLE_HASH)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootConfig<'a> {
    version: &'a str,
    asset_hash: String,
    panel_port: u16,
    security_entrance: &'a str,
    custom_domain: Option<&'a str>,
    ui_engine: &'a str,
}

fn escape_json_for_script_tag(json: &str) -> String {
    json.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

fn if_none_match_hits(req: &HttpRequest, etag: &str) -> bool {
    let Some(raw) = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let clean_target = etag.trim().trim_start_matches("W/");
    raw.split(',').any(|part| {
        let candidate = part.trim();
        candidate == "*" || candidate.trim_start_matches("W/") == clean_target
    })
}

fn has_matching_version_query(req: &HttpRequest, expected_tag: &str) -> bool {
    req.query_string().split('&').any(|pair| {
        pair.split_once('=')
            .map(|(k, v)| k == "v" && v == expected_tag)
            .unwrap_or(false)
    })
}

/// Renders the Pure-Rust native panel HTML shell with safe server-side boot metadata.
pub async fn serve_native_index(state: web::Data<AppState>) -> HttpResponse {
    let asset_tag = ui_bundle_version_tag();
    let boot = BootConfig {
        version: env!("CARGO_PKG_VERSION"),
        asset_hash: asset_tag.clone(),
        panel_port: state.config.port,
        security_entrance: state.config.security_entrance.as_str(),
        custom_domain: state.config.custom_domain.as_deref(),
        ui_engine: "pure-rust-native",
    };

    let boot_json = serde_json::to_string(&boot)
        .map(|raw| escape_json_for_script_tag(&raw))
        .unwrap_or_else(|_| "{}".to_string());

    let html = INDEX_HTML_TEMPLATE
        .replace("<!--FERROUS_BOOT_JSON-->", &boot_json)
        .replace("<!--FERROUS_ASSET_V-->", &asset_tag);

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header((header::CACHE_CONTROL, HTML_CACHE_CONTROL))
        .insert_header((UI_HEADER_NAME, UI_HEADER_VALUE))
        .body(html)
}

fn serve_static(
    req: &HttpRequest,
    content_type: &'static str,
    body: &'static str,
    hash: u64,
) -> HttpResponse {
    let etag = format!("\"fp-ui-{}-{:016x}\"", env!("CARGO_PKG_VERSION"), hash);
    let asset_tag = ui_bundle_version_tag();
    let cache_control = if has_matching_version_query(req, &asset_tag) {
        ASSET_CACHE_IMMUTABLE
    } else {
        ASSET_CACHE_REVALIDATE
    };

    if if_none_match_hits(req, &etag) {
        return HttpResponse::NotModified()
            .insert_header((header::CACHE_CONTROL, cache_control))
            .insert_header((header::ETAG, etag))
            .insert_header((header::VARY, "Accept-Encoding"))
            .insert_header((UI_HEADER_NAME, UI_HEADER_VALUE))
            .finish();
    }

    HttpResponse::Ok()
        .content_type(content_type)
        .insert_header((header::CACHE_CONTROL, cache_control))
        .insert_header((header::ETAG, etag))
        .insert_header((header::VARY, "Accept-Encoding"))
        .insert_header((UI_HEADER_NAME, UI_HEADER_VALUE))
        .body(body)
}

async fn serve_css(req: HttpRequest) -> HttpResponse {
    serve_static(&req, "text/css; charset=utf-8", PANEL_CSS, HASH_PANEL_CSS)
}

async fn serve_favicon(req: HttpRequest) -> HttpResponse {
    serve_static(
        &req,
        "image/svg+xml; charset=utf-8",
        FAVICON_SVG,
        HASH_FAVICON_SVG,
    )
}

async fn serve_app_js(req: HttpRequest) -> HttpResponse {
    serve_static(
        &req,
        "application/javascript; charset=utf-8",
        APP_JS,
        HASH_APP_JS,
    )
}

async fn serve_views_core_js(req: HttpRequest) -> HttpResponse {
    serve_static(
        &req,
        "application/javascript; charset=utf-8",
        VIEWS_CORE_JS,
        HASH_VIEWS_CORE_JS,
    )
}

async fn serve_views_infra_js(req: HttpRequest) -> HttpResponse {
    serve_static(
        &req,
        "application/javascript; charset=utf-8",
        VIEWS_INFRA_JS,
        HASH_VIEWS_INFRA_JS,
    )
}

async fn serve_views_security_js(req: HttpRequest) -> HttpResponse {
    serve_static(
        &req,
        "application/javascript; charset=utf-8",
        VIEWS_SECURITY_JS,
        HASH_VIEWS_SECURITY_JS,
    )
}

/// Registers Pure-Rust native UI asset endpoints (`/ui/*`).
pub fn configure_native_ui_assets(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/ui/panel.css").route(web::get().to(serve_css)))
        .service(web::resource("/ui/favicon.svg").route(web::get().to(serve_favicon)))
        .service(web::resource("/ui/app.js").route(web::get().to(serve_app_js)))
        .service(web::resource("/ui/views-core.js").route(web::get().to(serve_views_core_js)))
        .service(web::resource("/ui/views-infra.js").route(web::get().to(serve_views_infra_js)))
        .service(
            web::resource("/ui/views-security.js").route(web::get().to(serve_views_security_js)),
        );
}
