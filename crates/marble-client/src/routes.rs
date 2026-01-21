//! Application routes.

use yew_router::prelude::*;

/// Application routes.
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    /// Home page (placeholder for future main game).
    #[at("/")]
    Home,
    /// Debug pages index.
    #[at("/debug")]
    DebugIndex,
    /// Simple debug page with basic simulation.
    #[at("/debug/simple")]
    DebugSimple,
    /// 404 Not Found.
    #[not_found]
    #[at("/404")]
    NotFound,
}
