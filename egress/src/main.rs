mod azure_storage;
mod config;
mod kubernetes;
mod manifest;

use config::{AppConfig, JobConfig};
use axum::{
    body::StreamBody,
    extract::{BodyStream, FromRef, Path, State},
    http::HeaderValue,
    response::{IntoResponse, Redirect, Response},
    routing::{get, get_service, post},
    Router, Server,
};
use azure_storage::{AzureStorage, StorageServer};
use futures::stream::StreamExt;
use hyper::{HeaderMap, Method, StatusCode};
use kubernetes::KubernetesMediaServer;
use log::{error, info};
use manifest::ManifestServer;
use tokio_pipe::PipeWrite;
use tower_http::{
    cors::Any,
    services::{ServeDir, ServeFile},
};

#[derive(Clone)]
struct AppState {
    storage: AzureStorage,
    media: KubernetesMediaServer,
}

impl AppState {
    async fn new() -> Self {
        let config = AppConfig::new().unwrap();
        let storage = AzureStorage::new(config.clone());
        let job_config = JobConfig::new();
        let media = KubernetesMediaServer::new(config, job_config, storage).await;
        Self { storage, media }
    }
}

impl FromRef<AppState> for KubernetesMediaServer {
    fn from_ref(app_state: &AppState) -> KubernetesMediaServer {
        app_state.media.clone()
    }
}

const HLS_MIME_TYPE: &str = "application/vnd.apple.mpegurl";
const TS_MIME_TYPE: &str = "video/mp2t";

async fn get_variant_playlist(
    State(server): State<AppState>,
    Path((container, video)): Path<(String, String)>,
) -> (StatusCode, HeaderMap, String) {
    let mut headers = HeaderMap::new();
    if video.ends_with(".m3u8") {
        headers.append("Content-Type", HeaderValue::from_static(HLS_MIME_TYPE));
        let result = server.storage.get_video(&container, &video).await;
        if let Err(e) = result  {
            return (StatusCode::NOT_FOUND, headers, String::new());
        }
        let storage = result.unwrap();
        let manifest = ManifestServer::new(storage);
        let result = manifest.get_variant_playlist(video).await;
        return match result {
            Ok(playst) => (StatusCode::OK, headers, playst),
            _ => (StatusCode::NOT_FOUND, headers, "".into()),
        };
    }
    (StatusCode::NOT_FOUND, headers, "".into())
}

async fn get_media_playlist(
    State(server): State<AppState>,
    Path((container, video, level)): Path<(String, String, String)>,
) -> (StatusCode, HeaderMap, impl IntoResponse) {
    let mut headers = HeaderMap::new();
    if level.ends_with(".m3u8") && level.starts_with("level") {
        headers.append("Content-Type", HeaderValue::from_static(HLS_MIME_TYPE));
        let l = level.parse::<u32>().unwrap_or(0_u32);
        let result = server.storage.get_video(conatiner, video).await;
        let storage = match result  {
            Ok(s) =>s,
            _ => { return (StatusCode::NOT_FOUND, headers, String::new()); }
        };
        let manifest = ManifestServer::new(storage);
        let result = manifest
            .get_media_playlist(video, l)
            .await;
        return match result {
            Ok(playlist) => (StatusCode::OK, headers, playlist),
            _ => (StatusCode::NOT_FOUND, HeaderMap::new(), "Not found".into()),
        };
    }
    (StatusCode::NOT_FOUND, headers, String::new())
}

async fn get_media_segment(
    State(state): State<AppState>,
    Path((container, video, level, segment)): Path<(String, String, String, String)>,
) -> (StatusCode, HeaderMap, Response) {
    let mut headers = HeaderMap::new();

    let l = level[5..].parse::<u32>().unwrap();
    let s = segment[7..segment.len() - 3].parse::<u32>().unwrap();
    let result = state
        .media
        .get_media_segment(&container, &video, l, s)
        .await;
    match result {
        Ok(stream) => {
            headers.append("Content-Type", HeaderValue::from_static(TS_MIME_TYPE));
            let body = StreamBody::new(Box::into_pin(stream));
            (StatusCode::OK, headers, body.into_response())
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            headers,
            StreamBody::default().into_response(),
        ),
    }
}

async fn copy_body_to_pipe(mut stream: BodyStream, pipe: String) -> anyhow::Result<()> {
    let fd = pipe.parse::<i32>()?;
    let mut writer = PipeWrite::from_raw_fd_checked(fd)?;
    while let Some(chunk) = stream.next().await {
        let mut bytes = chunk?;
        info!("Read {} bytes from body", bytes.len());
        while !bytes.is_empty() {
            let size = writer.write(&bytes).await?;
            info!("wrote {} bytes to pipe {}", size, fd);
            bytes = bytes.slice(size..)
        }
    }
    Ok(())
}

async fn post_to_pipe(Path(pipe): Path<String>, stream: BodyStream) -> (StatusCode, Response) {
    let result = copy_body_to_pipe(stream, pipe).await;
    if result.is_ok() {
        return (StatusCode::ACCEPTED, StreamBody::default().into_response());
    }
    error!("Post to pipe failed! {:?}", result);
    (StatusCode::BAD_REQUEST, "".into_response())
}

const STATIC_DIR: &str = "./wwwroot/";
async fn handle_error(_err: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
async fn shutdown_signal() {
    // Wait for the CTRL+C signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    info!("Received Ctrl+C. Terminating...");
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::builder().init();

    let cors = tower_http::cors::CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_origin(Any);

    // build our application with a single route
    let state = AppState::new().await;
    let service = ServeDir::new(STATIC_DIR).not_found_service(ServeFile::new("wwwroot/hlsjs.html"));
    let app = Router::new()
        .with_state(state)
        .nest("/wwwroot", get_service(service).handle_error(handle_error))
        .route(
            "/",
            get(|| async { Redirect::permanent("/wwwroot/hlsjs.html") }),
        )
        .route("/:container/:video", get(get_variant_playlist))
        .route("/:container/:video/:level", get(get_media_playlist))
        .route("/:container/:video/:level/:segment", get(get_media_segment))
        .route("/pipe/:pipe", post(post_to_pipe))
        .layer(cors);

    // run it with hyper on localhost:3000
    let addr = std::env::var("BIND_ENDPOINT").unwrap_or_else(|_| "0.0.0.0:3000".into());
    info!("Starting the server on endpoint {}", addr);
    let server = Server::bind(&addr.parse().unwrap()).serve(app.into_make_service());
    server.with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}
