//! Debug pages index.

use crate::routes::Route;
use yew::prelude::*;
use yew_router::prelude::*;
use crate::pages::DebugGrpcPage;

/// Debug index page with links to debug pages.
#[function_component(DebugIndexPage)]
pub fn debug_index_page() -> Html {
    html! {
        <main class="page debug-index-page">
            <h1>{ "Debug Pages" }</h1>
            <ul class="debug-links">
                <li>
                    <Link<Route> to={Route::DebugGrpc}>
                        { "Grpc Debug" }
                    </Link<Route>>
                    <span class="link-desc">{ " - Basic gRPC client for testing the server's gRPC endpoints." }</span>
                </li>
            </ul>
        </main>
    }
}
