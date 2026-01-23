//! Main application component.

use crate::pages::{DebugIndexPage, DebugSimplePage, HomePage, NotFoundPage, PlayPage};
use crate::routes::Route;
use yew::prelude::*;
use yew_router::prelude::*;

/// Route switch function.
fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <HomePage /> },
        Route::Play { room_id } => html! { <PlayPage room_id={room_id} /> },
        Route::DebugIndex => html! { <DebugIndexPage /> },
        Route::DebugSimple => html! { <DebugSimplePage /> },
        Route::NotFound => html! { <NotFoundPage /> },
    }
}

/// Root application component with router.
#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}
