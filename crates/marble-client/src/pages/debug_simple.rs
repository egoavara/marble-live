//! Simple debug page with basic simulation.

use crate::components::{Controls, DebugPanel, GameCanvas};
use crate::state::AppState;
use yew::prelude::*;

/// Debug simple page - the original debug UI.
#[function_component(DebugSimplePage)]
pub fn debug_simple_page() -> Html {
    let app_state = use_reducer(AppState::new);

    html! {
        <ContextProvider<crate::state::AppStateContext> context={app_state}>
            <main class="page debug-simple-page">
                <header class="page-header">
                    <h1>{ "Debug: Simple Simulation" }</h1>
                </header>
                <div class="game-area">
                    <div class="canvas-container">
                        <GameCanvas />
                    </div>
                    <aside class="sidebar">
                        <Controls />
                        <DebugPanel />
                    </aside>
                </div>
            </main>
        </ContextProvider<crate::state::AppStateContext>>
    }
}
