//! Local HTTP server: JSON API plus the embedded UI.

use crate::workspace;
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use rust_embed::RustEmbed;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "ui/dist"]
struct Assets;

struct App {
    roots: Vec<PathBuf>,
    port: u16,
}

type Shared = Arc<App>;

async fn snapshot(app: &Shared) -> workspace::Snapshot {
    let roots = app.roots.clone();
    tokio::task::spawn_blocking(move || workspace::build(&roots))
        .await
        .expect("workspace build panicked")
}

async fn get_workspace(State(app): State<Shared>) -> Response {
    axum::Json(snapshot(&app).await.workspace).into_response()
}

async fn get_doc(State(app): State<Shared>, Path(id): Path<String>) -> Response {
    match workspace::doc(&snapshot(&app).await, &id) {
        Some(doc) => axum::Json(doc).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn post_read(State(app): State<Shared>, Path(id): Path<String>) -> Response {
    reply(workspace::mark_read(&snapshot(&app).await, &id))
}

async fn post_accept(
    State(app): State<Shared>,
    Path((id, hunk)): Path<(String, String)>,
) -> Response {
    reply(workspace::accept(&snapshot(&app).await, &id, &hunk))
}

async fn post_revert(
    State(app): State<Shared>,
    Path((id, hunk)): Path<(String, String)>,
) -> Response {
    reply(workspace::revert(&snapshot(&app).await, &id, &hunk))
}

async fn post_undo(State(app): State<Shared>, Path(id): Path<String>) -> Response {
    reply(workspace::undo(&snapshot(&app).await, &id))
}

fn reply(result: anyhow::Result<()>) -> Response {
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::CONFLICT, e.to_string()).into_response(),
    }
}

async fn asset(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path).or_else(|| Assets::get("index.html")) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or(mime_guess::mime::TEXT_HTML);
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                Body::from(file.data),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Only answer requests addressed to this machine (blocks DNS rebinding),
/// and require a custom header on writes so other sites cannot forge them.
async fn guard(
    State(app): State<Shared>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let allowed = [
        format!("127.0.0.1:{}", app.port),
        format!("localhost:{}", app.port),
    ];
    if !allowed.iter().any(|a| a == host) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if req.method() != axum::http::Method::GET && headers.get("x-mieruka").is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(req).await
}

pub async fn serve(roots: Vec<PathBuf>, port: u16, open_browser: bool) -> anyhow::Result<()> {
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(_) => tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?,
    };
    let port = listener.local_addr()?.port();
    let app: Shared = Arc::new(App { roots, port });
    let router = Router::new()
        .route("/api/workspace", get(get_workspace))
        .route("/api/doc/{id}", get(get_doc))
        .route("/api/read/{id}", post(post_read))
        .route("/api/accept/{id}/{hunk}", post(post_accept))
        .route("/api/revert/{id}/{hunk}", post(post_revert))
        .route("/api/undo/{id}", post(post_undo))
        .fallback(asset)
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app);

    let url = format!("http://127.0.0.1:{port}/");
    println!("mieruka is running at {url}  (Ctrl+C to stop)");
    if open_browser {
        let _ = open::that(&url);
    }
    axum::serve(listener, router).await?;
    Ok(())
}
