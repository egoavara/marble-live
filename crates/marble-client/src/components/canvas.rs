//! Canvas component for rendering the game.
//!
//! NOTE: This is a legacy component. New code should use BevyProvider + EditorCanvas.
//! This component is kept for backwards compatibility but rendering is now handled by Bevy.

use yew::prelude::*;

/// Canvas width in pixels.
pub const CANVAS_WIDTH: u32 = 800;
/// Canvas height in pixels.
pub const CANVAS_HEIGHT: u32 = 600;

/// Properties for the GameCanvas component.
#[derive(Properties, PartialEq)]
pub struct GameCanvasProps {}

/// Game canvas component - legacy stub.
/// Bevy now handles all rendering via BevyProvider.
#[function_component(GameCanvas)]
pub fn game_canvas(_props: &GameCanvasProps) -> Html {
    html! {
        <canvas
            id="game-canvas"
            width={CANVAS_WIDTH.to_string()}
            height={CANVAS_HEIGHT.to_string()}
        />
    }
}
