//! Marble-Live Server
//!
//! Axum backend with gRPC-Web, matchbox signaling, and SPA serving.
//! Static files are embedded in the binary via rust-embed.

mod room_service;
mod room_state;

use std::net::SocketAddr;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Router,
};
use http::{header, Method};
use marble_proto::room::room_service_server::RoomServiceServer;
use matchbox_signaling::SignalingServer;
use room_service::RoomServiceImpl;
use room_state::RoomStore;
use rust_embed::Embed;
use tonic::service::Routes;
use tonic_web::GrpcWebLayer;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Embedded static files from dist/ directory
#[derive(Embed)]
#[folder = "../../dist/"]
struct Assets;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    // Room state store
    let room_store = RoomStore::new();

    // gRPC service
    // Use localhost for browser-accessible signaling URL
    let signaling_base_url = format!("ws://localhost:{}/signaling", addr.port());
    let room_service = RoomServiceImpl::new(room_store, signaling_base_url);
    let grpc_router = Routes::new(RoomServiceServer::new(room_service))
        .into_axum_router()
        .layer(GrpcWebLayer::new());

    // CORS layer for gRPC-Web
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            "x-grpc-web".parse().unwrap(),
            "grpc-timeout".parse().unwrap(),
        ])
        .expose_headers([
            "grpc-status".parse().unwrap(),
            "grpc-message".parse().unwrap(),
        ]);

    // App router (gRPC + embedded SPA)
    let app_router = Router::new()
        .nest("/grpc", grpc_router)
        .fallback(serve_embedded)
        .layer(cors);

    // Build signaling server with integrated app router
    let signaling_server = SignalingServer::full_mesh_builder(addr)
        .cors()
        .trace()
        .on_peer_connected(|peer_id| {
            tracing::info!("Peer connected: {peer_id}");
        })
        .on_peer_disconnected(|peer_id| {
            tracing::info!("Peer disconnected: {peer_id}");
        })
        .build_with(|signaling_router| {
            Router::new()
                .nest("/signaling", signaling_router)
                .merge(app_router)
        });

    tracing::info!("Server listening on {addr}");
    tracing::info!("  - gRPC-Web: http://{addr}/grpc/room.RoomService/*");
    tracing::info!("  - Signaling: ws://{addr}/signaling/{{room_id}}");
    tracing::info!("  - SPA (embedded): http://{addr}/");

    signaling_server.serve().await?;

    Ok(())
}

/// Serve embedded static files with SPA fallback
async fn serve_embedded(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for client-side routing
            match Assets::get("index.html") {
                Some(content) => {
                    let mime = mime_guess::from_path("index.html").first_or_octet_stream();
                    (
                        [(header::CONTENT_TYPE, mime.as_ref())],
                        content.data.into_owned(),
                    )
                        .into_response()
                }
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}
