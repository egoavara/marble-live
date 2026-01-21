//! 404 Not Found page.

use crate::routes::Route;
use yew::prelude::*;
use yew_router::prelude::*;

/// 404 Not Found page.
#[function_component(NotFoundPage)]
pub fn not_found_page() -> Html {
    html! {
        <main class="page not-found-page">
            <h1>{ "404" }</h1>
            <p>{ "Page not found" }</p>
            <Link<Route> to={Route::Home}>{ "Go to Home" }</Link<Route>>
        </main>
    }
}
