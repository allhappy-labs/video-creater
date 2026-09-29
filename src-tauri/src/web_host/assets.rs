use std::path::Path;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

pub fn asset_router(assets_dir: &Path) -> Router {
    let index = assets_dir.join("index.html");
    Router::new()
        .fallback_service(ServeDir::new(assets_dir).not_found_service(ServeFile::new(index)))
}
