//! Main application component.

use marble_core::RouletteConfig;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::game_view::GAME_VIEW_CANVAS_ID;
use crate::hooks::{is_bevy_app_running, prepare_new_room, BevyProvider};
use crate::pages::{
    DebugGrpcPage, DebugIndexPage, DebugP2pPage, EditorPage, HomePage, NotFoundPage, PanicPage,
    PlayPage,
};
use crate::routes::Route;

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

/// Component that manages canvas visibility based on current route.
#[function_component(CanvasVisibilityManager)]
fn canvas_visibility_manager() -> Html {
    let route = use_route::<Route>();

    // Determine if we're on a play page
    let is_play_page = matches!(route, Some(Route::Play { .. }));

    // Track previous room_id for room transitions
    let prev_room_id = use_mut_ref(|| None::<String>);

    // Get room_id if on play page
    let room_id = match &route {
        Some(Route::Play { room_id }) => Some(room_id.clone()),
        _ => None,
    };

    // Handle room transitions
    {
        let room_id = room_id.clone();
        let prev_room_id = prev_room_id.clone();

        use_effect_with(room_id.clone(), move |room_id| {
            if let Some(current_room) = room_id {
                let prev = prev_room_id.borrow().clone();

                // If room changed, prepare for new room
                if prev.is_some() && prev.as_ref() != Some(current_room) {
                    tracing::info!(
                        "Room transition detected: {:?} -> {}",
                        prev,
                        current_room
                    );

                    if is_bevy_app_running() {
                        let config_json = serde_json::to_string(&RouletteConfig::default_classic())
                            .unwrap_or_else(|_| "{}".to_string());

                        if let Err(e) = prepare_new_room(&config_json) {
                            tracing::error!("Failed to prepare new room: {:?}", e);
                        }
                    }
                }

                // Update previous room_id
                *prev_room_id.borrow_mut() = Some(current_room.clone());
            }
        });
    }

    // Apply visibility class to canvas via JavaScript
    {
        use_effect_with(is_play_page, move |is_play| {
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    if let Some(canvas) = document.get_element_by_id(GAME_VIEW_CANVAS_ID) {
                        let style = canvas
                            .dyn_ref::<web_sys::HtmlElement>()
                            .map(|el| el.style());

                        if let Some(style) = style {
                            if *is_play {
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

use wasm_bindgen::JsCast;

/// Inner app component with BevyProvider wrapping everything.
#[function_component(AppWithBevy)]
fn app_with_bevy() -> Html {
    let route = use_route::<Route>();
    let bevy_ever_initialized = use_state(|| false);

    // Determine if we're on a play page
    let is_play_page = matches!(route, Some(Route::Play { .. }));

    // Track if Bevy was ever initialized
    {
        let bevy_ever_initialized = bevy_ever_initialized.clone();
        let is_play_page = is_play_page;

        use_effect_with(is_play_page, move |is_play| {
            if *is_play && !*bevy_ever_initialized {
                bevy_ever_initialized.set(true);
            }
        });
    }

    // Get default config for initial load
    let config_json = serde_json::to_string(&RouletteConfig::default_classic())
        .unwrap_or_else(|_| "{}".to_string());

    // Only render BevyProvider if we've ever been on a play page
    let should_render_bevy = is_play_page || *bevy_ever_initialized;

    if should_render_bevy {
        html! {
            <BevyProvider
                canvas_id={GAME_VIEW_CANVAS_ID}
                config_json={config_json}
                editor_mode={false}
            >
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
