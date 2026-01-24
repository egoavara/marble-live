//! Application routes.

use yew_router::prelude::*;

/// Application routes.
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    /// Home page.
    #[at("/")]
    Home,
    /// Play page with room ID.
    #[at("/play/:room_id")]
    Play { room_id: String },
    /// Panic page shown when WASM panic occurs.
    #[at("/panic")]
    Panic,
    /// Debug page for gRPC calls.
    #[at("/debug")]
    Debug,
    /// Debug page for gRPC calls.
    #[at("/debug/grpccall")]
    DebugGrpc,
    /// 404 Not Found.
    #[not_found]
    #[at("/404")]
    NotFound,
}
