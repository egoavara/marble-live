//! Main application component.

use crate::pages::{
    DebugGrpcPage, DebugIndexPage, DebugP2pPage, HomePage, NotFoundPage, PanicPage, PlayPage,
};
use crate::routes::Route;
use yew::prelude::*;
use yew_router::prelude::*;

/// Route switch function.
fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <HomePage /> },
        Route::Play { room_id } => html! { <PlayPage room_id={room_id} /> },
        Route::Panic => html! { <PanicPage /> },
        Route::NotFound => html! { <NotFoundPage /> },
        Route::Debug => html! { <DebugIndexPage /> },
        Route::DebugGrpc => html! { <DebugGrpcPage /> },
        Route::DebugP2p => html! { <DebugP2pPage /> },
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
