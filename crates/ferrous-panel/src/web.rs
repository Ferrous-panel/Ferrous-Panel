use actix_web::{web, HttpRequest, HttpResponse};
use rust_embed::Embed;
use std::path::Path;

/// Prebuilt React frontend static assets from repository `/dist`.
///
/// `allow_missing = true` permits building/testing in Rust-only environments
/// before `npm run build` is executed; release binaries should be built after `npm run build`.
#[derive(Embed)]
#[folder = "../../dist/"]
#[allow_missing = true]
struct DistAssets;

fn disk_fallback_response(path: &str) -> Option<HttpResponse> {
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

fn embedded_response(path: &str) -> Option<HttpResponse> {
    if let Some(asset) = DistAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Some(
            HttpResponse::Ok()
                .content_type(mime.as_ref())
                .body(asset.data.into_owned()),
        );
    }
    disk_fallback_response(path)
}

async fn serve_index() -> HttpResponse {
    embedded_response("index.html").unwrap_or_else(|| {
        HttpResponse::ServiceUnavailable()
            .content_type("text/plain; charset=utf-8")
            .body("Frontend assets not embedded yet. Run `bun --bun run build` before `cargo build --release`.")
    })
}

async fn serve_asset_or_spa(req: HttpRequest) -> HttpResponse {
    let clean_path = req.path().trim_start_matches('/');
    if clean_path.is_empty() {
        return serve_index().await;
    }

    if let Some(resp) = embedded_response(clean_path) {
        return resp;
    }

    // Do not fall back to index.html for `/api/*` or missing static bundle assets.
    if clean_path == "api"
        || clean_path.starts_with("api/")
        || clean_path.starts_with("assets/")
        || Path::new(clean_path).extension().is_some()
    {
        return HttpResponse::NotFound().finish();
    }

    serve_index().await
}

/// Registers embedded static asset serving, Pure-Rust FerrousAdmin (`/phpadmin/`),
/// and SPA client-route fallback.
pub fn configure_web(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/").route(web::get().to(serve_index)))
        .service(web::resource("/phpadmin").route(web::get().to(crate::pma_web::serve_ferrous_admin)))
        .service(web::resource("/phpadmin/").route(web::get().to(crate::pma_web::serve_ferrous_admin)))
        .service(web::resource("/phpmyadmin").route(web::get().to(crate::pma_web::serve_ferrous_admin)))
        .service(web::resource("/phpmyadmin/").route(web::get().to(crate::pma_web::serve_ferrous_admin)))
        .service(web::resource("/{tail:.*}").route(web::get().to(serve_asset_or_spa)));
}
