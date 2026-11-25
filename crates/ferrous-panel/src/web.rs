use crate::state::AppState;
use crate::ui::{configure_native_ui_assets, serve_native_index};
use actix_web::{web, HttpRequest, HttpResponse};
use rust_embed::Embed;
use std::path::Path;

/// Optional legacy Vite `dist/` assets (only used if `FERROUS_LEGACY_DIST_UI=1` is set).
/// By default, FerrousPanel serves its built-in Pure-Rust Native UI (`crate::ui`)
/// directly from the binary with zero Node.js / npm / Vite build dependency.
#[derive(Embed)]
#[folder = "../../dist/"]
#[allow_missing = true]
struct LegacyDistAssets;

fn legacy_dist_enabled() -> bool {
    std::env::var("FERROUS_LEGACY_DIST_UI")
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn legacy_disk_response(path: &str) -> Option<HttpResponse> {
    let clean = path.trim_start_matches('/');
    if clean.is_empty() || clean.contains("..") || clean.contains('\0') {
        return None;
    }
    for base_str in ["/var/lib/ferrous-panel/dist", "/usr/local/src/ferrous-panel/dist"] {
        let base = Path::new(base_str);
        if !base.is_dir() {
            continue;
        }
        let Ok(canon_base) = std::fs::canonicalize(base) else {
            continue;
        };
        let candidate = base.join(clean);
        let Ok(canon_target) = std::fs::canonicalize(&candidate) else {
            continue;
        };
        if !canon_target.starts_with(&canon_base) || !canon_target.is_file() {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&canon_target) {
            let mime = mime_guess::from_path(&canon_target).first_or_octet_stream();
            return Some(
                HttpResponse::Ok()
                    .content_type(mime.as_ref())
                    .body(bytes),
            );
        }
    }
    None
}

fn legacy_embedded_response(path: &str) -> Option<HttpResponse> {
    if let Some(asset) = LegacyDistAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Some(
            HttpResponse::Ok()
                .content_type(mime.as_ref())
                .body(asset.data.into_owned()),
        );
    }
    legacy_disk_response(path)
}

async fn serve_index(state: web::Data<AppState>) -> HttpResponse {
    if legacy_dist_enabled() {
        if let Some(resp) = legacy_embedded_response("index.html") {
            return resp;
        }
    }
    serve_native_index(state).await
}

async fn serve_asset_or_spa(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let clean_path = req.path().trim_start_matches('/');
    if clean_path.is_empty() {
        return serve_index(state).await;
    }

    if legacy_dist_enabled() {
        if let Some(resp) = legacy_embedded_response(clean_path) {
            return resp;
        }
    }

    // Never fall back to HTML shell for `/api/*`, `/ui/*`, or missing static assets.
    if clean_path == "api"
        || clean_path.starts_with("api/")
        || clean_path.starts_with("ui/")
        || clean_path.starts_with("assets/")
        || Path::new(clean_path).extension().is_some()
    {
        return HttpResponse::NotFound().finish();
    }

    serve_index(state).await
}

/// Registers Pure-Rust Native Panel UI (`/`, `/ui/*`), Pure-Rust FerrousAdmin (`/phpadmin/`),
/// Official phpMyAdmin reverse proxy (`/phpmyadmin/*`), and SPA client-route fallback (e.g. `/panel`).
pub fn configure_web(cfg: &mut web::ServiceConfig) {
    const PMA_UPLOAD_LIMIT: usize = 128 * 1024 * 1024;
    cfg.configure(configure_native_ui_assets)
        .service(web::resource("/").route(web::get().to(serve_index)))
        .service(web::resource("/phpadmin").route(web::get().to(crate::pma_web::serve_ferrous_admin)))
        .service(web::resource("/phpadmin/").route(web::get().to(crate::pma_web::serve_ferrous_admin)))
        .service(
            web::resource("/phpmyadmin")
                .route(web::get().to(crate::pma_web::redirect_phpmyadmin_slash)),
        )
        .service(
            web::resource("/phpmyadmin/")
                .app_data(web::PayloadConfig::new(PMA_UPLOAD_LIMIT))
                .default_service(web::to(crate::pma_web::serve_official_phpmyadmin)),
        )
        .service(
            web::resource("/phpmyadmin/{tail:.*}")
                .app_data(web::PayloadConfig::new(PMA_UPLOAD_LIMIT))
                .default_service(web::to(crate::pma_web::serve_official_phpmyadmin)),
        )
        .service(web::resource("/{tail:.*}").route(web::get().to(serve_asset_or_spa)));
}
