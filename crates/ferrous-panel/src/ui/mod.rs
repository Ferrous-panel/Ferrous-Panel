use crate::state::AppState;
use actix_web::http::header;
use actix_web::{web, HttpResponse};
use serde::Serialize;

const INDEX_HTML_TEMPLATE: &str = include_str!("assets/index.html");
const PANEL_CSS: &str = include_str!("assets/panel.css");
const FAVICON_SVG: &str = include_str!("assets/favicon.svg");
const APP_JS: &str = include_str!("assets/app.js");
const VIEWS_CORE_JS: &str = include_str!("assets/views-core.js");
const VIEWS_INFRA_JS: &str = include_str!("assets/views-infra.js");
const VIEWS_SECURITY_JS: &str = include_str!("assets/views-security.js");

const ASSET_CACHE_CONTROL: &str = "public, max-age=300, must-revalidate";
const HTML_CACHE_CONTROL: &str = "no-store, max-age=0";
const UI_HEADER_NAME: &str = "X-Ferrous-UI";
const UI_HEADER_VALUE: &str = "pure-rust-native";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootConfig<'a> {
    version: &'a str,
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

/// Renders the Pure-Rust native panel HTML shell with safe server-side boot metadata.
pub async fn serve_native_index(state: web::Data<AppState>) -> HttpResponse {
    let boot = BootConfig {
        version: env!("CARGO_PKG_VERSION"),
        panel_port: state.config.port,
        security_entrance: state.config.security_entrance.as_str(),
        custom_domain: state.config.custom_domain.as_deref(),
        ui_engine: "pure-rust-native",
    };

    let boot_json = serde_json::to_string(&boot)
        .map(|raw| escape_json_for_script_tag(&raw))
        .unwrap_or_else(|_| "{}".to_string());

    let html = INDEX_HTML_TEMPLATE.replace("<!--FERROUS_BOOT_JSON-->", &boot_json);

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header((header::CACHE_CONTROL, HTML_CACHE_CONTROL))
        .insert_header((UI_HEADER_NAME, UI_HEADER_VALUE))
        .body(html)
}

fn serve_static(content_type: &'static str, body: &'static str) -> HttpResponse {
    let etag = format!("\"fp-ui-{}-{}\"", env!("CARGO_PKG_VERSION"), body.len());
    HttpResponse::Ok()
        .content_type(content_type)
        .insert_header((header::CACHE_CONTROL, ASSET_CACHE_CONTROL))
        .insert_header((header::ETAG, etag))
        .insert_header((UI_HEADER_NAME, UI_HEADER_VALUE))
        .body(body)
}

async fn serve_css() -> HttpResponse {
    serve_static("text/css; charset=utf-8", PANEL_CSS)
}

async fn serve_favicon() -> HttpResponse {
    serve_static("image/svg+xml; charset=utf-8", FAVICON_SVG)
}

async fn serve_app_js() -> HttpResponse {
    serve_static("application/javascript; charset=utf-8", APP_JS)
}

async fn serve_views_core_js() -> HttpResponse {
    serve_static("application/javascript; charset=utf-8", VIEWS_CORE_JS)
}

async fn serve_views_infra_js() -> HttpResponse {
    serve_static("application/javascript; charset=utf-8", VIEWS_INFRA_JS)
}

async fn serve_views_security_js() -> HttpResponse {
    serve_static("application/javascript; charset=utf-8", VIEWS_SECURITY_JS)
}

/// Registers Pure-Rust native UI asset endpoints (`/ui/*`).
pub fn configure_native_ui_assets(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/ui/panel.css").route(web::get().to(serve_css)))
        .service(web::resource("/ui/favicon.svg").route(web::get().to(serve_favicon)))
        .service(web::resource("/ui/app.js").route(web::get().to(serve_app_js)))
        .service(web::resource("/ui/views-core.js").route(web::get().to(serve_views_core_js)))
        .service(web::resource("/ui/views-infra.js").route(web::get().to(serve_views_infra_js)))
        .service(web::resource("/ui/views-security.js").route(web::get().to(serve_views_security_js)));
}
