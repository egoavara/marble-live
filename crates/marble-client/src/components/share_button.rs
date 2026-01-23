//! Share button component for copying room URL.

use crate::p2p::state::P2PStateContext;
use wasm_bindgen_futures::JsFuture;
use yew::prelude::*;

/// Share button that copies room URL to clipboard.
#[function_component(ShareButton)]
pub fn share_button() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");
    let copy_feedback = use_state(|| false);

    let on_click = {
        let room_id = state.room_id.clone();
        let copy_feedback = copy_feedback.clone();
        Callback::from(move |_| {
            let room_id = room_id.clone();
            let copy_feedback = copy_feedback.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(window) = web_sys::window() {
                    let location = window.location();
                    let origin = location.origin().unwrap_or_default();
                    let full_url = format!("{}/play/{}", origin, room_id);

                    let clipboard = window.navigator().clipboard();
                    let _ = JsFuture::from(clipboard.write_text(&full_url)).await;
                    copy_feedback.set(true);
                    // Reset feedback after 2 seconds
                    gloo::timers::callback::Timeout::new(2000, move || {
                        copy_feedback.set(false);
                    })
                    .forget();
                }
            });
        })
    };

    html! {
        <button
            class={classes!("share-btn", (*copy_feedback).then_some("copied"))}
            onclick={on_click}
            title="Copy room link"
        >
            { if *copy_feedback {
                html! { <span class="share-icon">{ "✓" }</span> }
            } else {
                html! { <span class="share-icon">{ "🔗" }</span> }
            }}
        </button>
    }
}
