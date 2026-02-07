//! Main application component.

use marble_core::RouletteConfig;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::hooks::{BevyProvider, init_editor_mode, init_game_mode, send_command};
use crate::pages::{
    DebugGrpcPage, DebugIndexPage, DebugP2pPage, EditorPage, HomePage, NotFoundPage, PanicPage,
    PlayPage,
};
use crate::routes::Route;

/// Unified Canvas ID for all Bevy rendering (game and editor).
pub const BEVY_CANVAS_ID: &str = "bevy-canvas";

/// Route switch function.
fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <HomePage /> },
        Route::Play { room_id } => html! { <PlayPage room_id={room_id} /> },
        Route::Editor => html! { <EditorPage /> },
        Route::Panic => html! { <PanicPage /> },
        Route::NotFound => html! { <NotFoundPage /> },
        Route::Debug => html! { <DebugIndexPage /> },
        Route::DebugGrpc => html! { <DebugGrpcPage /> },
        Route::DebugP2p => html! { <DebugP2pPage /> },
    }
}

/// Returns true if the given route needs a Bevy canvas.
fn route_needs_bevy(route: &Option<Route>) -> bool {
    matches!(route, Some(Route::Play { .. }) | Some(Route::Editor))
}

/// Component that manages canvas visibility and route-based mode transitions.
#[function_component(CanvasVisibilityManager)]
fn canvas_visibility_manager() -> Html {
    let route = use_route::<Route>();

    let needs_bevy = route_needs_bevy(&route);

    // Route-based mode transition
    // Commands are queued into Arc<Mutex<VecDeque>> CommandQueue,
    // so they can be safely sent before the Bevy app starts.
    // The app will process them on its first frames.
    {
        let route = route.clone();

        use_effect_with(route, move |route| {
            let config_json = serde_json::to_string(&RouletteConfig::default_classic())
                .unwrap_or_else(|_| "{}".to_string());

            match route {
                Some(Route::Play { .. }) => {
                    tracing::info!("[app] Route::Play -> init_game_mode");
                    if let Err(e) = init_game_mode(&config_json) {
                        tracing::error!("Failed to init game mode: {:?}", e);
                    }
                }
                Some(Route::Editor) => {
                    tracing::info!("[app] Route::Editor -> init_editor_mode");
                    if let Err(e) = init_editor_mode(&config_json) {
                        tracing::error!("Failed to init editor mode: {:?}", e);
                    }
                }
                _ => {
                    tracing::info!("[app] Non-bevy route -> clear_mode");
                    let _ = send_command(r#"{"type":"clear_mode"}"#);
                }
            }
        });
    }

    // Apply visibility to canvas via JavaScript
    {
        use_effect_with(needs_bevy, move |show| {
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    if let Some(canvas) = document.get_element_by_id(BEVY_CANVAS_ID) {
                        let style = canvas
                            .dyn_ref::<web_sys::HtmlElement>()
                            .map(|el| el.style());

                        if let Some(style) = style {
                            if *show {
                                let _ = style.set_property("visibility", "visible");
                                let _ = style.set_property("pointer-events", "auto");
                            } else {
                                let _ = style.set_property("visibility", "hidden");
                                let _ = style.set_property("pointer-events", "none");
                            }
                        }
                    }
                }
            }
        });
    }

    html! {}
}

/// Inner app component with BevyProvider wrapping everything.
///
/// BevyProvider is initialized on first visit to a Play or Editor page.
/// Once initialized, it persists across all route changes.
/// Mode switching is handled dynamically via commands.
#[function_component(AppWithBevy)]
fn app_with_bevy() -> Html {
    let route = use_route::<Route>();
    let bevy_ever_initialized = use_state(|| false);

    let needs_bevy = route_needs_bevy(&route);

    // Track if Bevy was ever initialized
    {
        let bevy_ever_initialized = bevy_ever_initialized.clone();

        use_effect_with(needs_bevy, move |needs| {
            if *needs && !*bevy_ever_initialized {
                bevy_ever_initialized.set(true);
            }
        });
    }

    let should_render_bevy = needs_bevy || *bevy_ever_initialized;

    if should_render_bevy {
        html! {
            <BevyProvider canvas_id={BEVY_CANVAS_ID}>
                <CanvasVisibilityManager />
                <Switch<Route> render={switch} />
            </BevyProvider>
        }
    } else {
        html! {
            <Switch<Route> render={switch} />
        }
    }
}

/// Root application component with router.
#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <AppWithBevy />
        </BrowserRouter>
    }
}
