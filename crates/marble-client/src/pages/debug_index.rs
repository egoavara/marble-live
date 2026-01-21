//! Debug pages index.

use crate::routes::Route;
use yew::prelude::*;
use yew_router::prelude::*;

/// Debug index page with links to debug pages.
#[function_component(DebugIndexPage)]
pub fn debug_index_page() -> Html {
    html! {
        <main class="page debug-index-page">
            <h1>{ "Debug Pages" }</h1>
            <ul class="debug-links">
                <li>
                    <Link<Route> to={Route::DebugSimple}>
                        { "Simple Simulation" }
                    </Link<Route>>
                    <span class="link-desc">{ " - Basic marble roulette with debug UI" }</span>
                </li>
                <li>
                    <Link<Route> to={Route::DebugConnTest}>
                        { "Connection Test" }
                    </Link<Route>>
                    <span class="link-desc">{ " - P2P connection and state sync test" }</span>
                </li>
            </ul>
        </main>
    }
}
