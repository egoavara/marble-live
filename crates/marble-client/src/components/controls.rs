//! Control buttons component.

use crate::state::{AppAction, AppStateContext};
use yew::prelude::*;

/// Properties for the Controls component.
#[derive(Properties, PartialEq)]
pub struct ControlsProps {}

/// Control buttons for starting/stopping/resetting the simulation.
#[function_component(Controls)]
pub fn controls(_props: &ControlsProps) -> Html {
    let app_state = use_context::<AppStateContext>().expect("AppStateContext not found");

    let on_start = {
        let app_state = app_state.clone();
        Callback::from(move |_: MouseEvent| {
            app_state.dispatch(AppAction::Start);
        })
    };

    let on_stop = {
        let app_state = app_state.clone();
        Callback::from(move |_: MouseEvent| {
            app_state.dispatch(AppAction::Stop);
        })
    };

    let on_reset = {
        let app_state = app_state.clone();
        Callback::from(move |_: MouseEvent| {
            app_state.dispatch(AppAction::Reset);
        })
    };

    let is_running = app_state.is_running;

    html! {
        <div class="controls">
            if !is_running {
                <button class="btn btn-start" onclick={on_start}>
                    { "Start" }
                </button>
            }
            if is_running {
                <button class="btn btn-stop" onclick={on_stop}>
                    { "Stop" }
                </button>
            }
            <button class="btn btn-reset" onclick={on_reset}>
                { "Reset" }
            </button>
        </div>
    }
}
