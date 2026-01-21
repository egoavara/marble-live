//! Canvas component for rendering the game.

use crate::renderer::CanvasRenderer;
use crate::state::{AppAction, AppStateContext};
use gloo::timers::callback::Interval;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

/// Canvas width in pixels.
pub const CANVAS_WIDTH: u32 = 800;
/// Canvas height in pixels.
pub const CANVAS_HEIGHT: u32 = 600;

/// Properties for the GameCanvas component.
#[derive(Properties, PartialEq)]
pub struct GameCanvasProps {}

/// Game canvas component that renders the roulette and marbles.
#[function_component(GameCanvas)]
pub fn game_canvas(_props: &GameCanvasProps) -> Html {
    let app_state = use_context::<AppStateContext>().expect("AppStateContext not found");
    let canvas_ref = use_node_ref();
    let renderer_ref = use_mut_ref(|| None::<CanvasRenderer>);

    // Initialize canvas and renderer
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();

        use_effect_with(canvas_ref.clone(), move |canvas_ref| {
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                canvas.set_width(CANVAS_WIDTH);
                canvas.set_height(CANVAS_HEIGHT);

                if let Ok(renderer) = CanvasRenderer::new(&canvas) {
                    *renderer_ref.borrow_mut() = Some(renderer);
                }
            }

            || ()
        });
    }

    // Game loop - only dispatch ticks, rendering happens in separate effect
    {
        let app_state = app_state.clone();

        use_effect_with(app_state.is_running, move |is_running| {
            let is_running = *is_running;
            let interval = if is_running {
                let app_state = app_state.clone();

                Some(Interval::new(16, move || {
                    app_state.dispatch(AppAction::Tick);
                }))
            } else {
                None
            };

            move || drop(interval)
        });
    }

    // Render whenever game state changes (frame number changes)
    {
        let renderer_ref = renderer_ref.clone();
        let game_state = app_state.game_state.clone();

        use_effect(move || {
            if let Some(renderer) = renderer_ref.borrow().as_ref() {
                renderer.render(&game_state);
            }
            || ()
        });
    }

    html! {
        <canvas
            ref={canvas_ref}
            id="game-canvas"
            width={CANVAS_WIDTH.to_string()}
            height={CANVAS_HEIGHT.to_string()}
        />
    }
}
