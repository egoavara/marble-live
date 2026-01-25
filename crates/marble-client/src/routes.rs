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
    /// Map editor page.
    #[at("/editor")]
    Editor,
    /// Panic page shown when WASM panic occurs.
    #[at("/panic")]
    Panic,
    /// Debug page index.
    #[at("/debug")]
    Debug,
    /// Debug page for gRPC calls.
    #[at("/debug/grpccall")]
    DebugGrpc,
    /// Debug page for P2P testing.
    #[at("/debug/p2p")]
    DebugP2p,
    /// 404 Not Found.
    #[not_found]
    #[at("/404")]
    NotFound,
}
