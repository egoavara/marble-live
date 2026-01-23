//! Home page with lobby functionality.

use crate::components::{Layout, WelcomeModal};
use crate::routes::Route;
use crate::storage::UserSettings;
use uuid::Uuid;
use yew::prelude::*;
use yew_router::prelude::*;

/// Extract room ID from input, handling both plain IDs and full URLs.
/// - If input is a URL like `http://localhost:3000/play/uuid`, extracts the UUID
/// - If input is just a UUID, returns it as-is
fn extract_room_id(input: &str) -> String {
    let trimmed = input.trim();

    // Check if it's a URL containing /play/
    if let Some(play_idx) = trimmed.find("/play/") {
        let after_play = &trimmed[play_idx + 6..]; // Skip "/play/"
        // Take only the UUID part (stop at next / or ? or end)
        let end_idx = after_play
            .find(|c| c == '/' || c == '?' || c == '#')
            .unwrap_or(after_play.len());
        return after_play[..end_idx].to_string();
    }

    // Return as-is if not a URL
    trimmed.to_string()
}

/// Home page component.
#[function_component(HomePage)]
pub fn home_page() -> Html {
    let navigator = use_navigator().unwrap();
    let show_welcome_modal = use_state(|| !UserSettings::exists());
    let room_id_input = use_state(String::new);

    let on_welcome_complete = {
        let show_welcome_modal = show_welcome_modal.clone();
        Callback::from(move |_settings: UserSettings| {
            show_welcome_modal.set(false);
        })
    };

    let on_start_race = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            let room_id = Uuid::new_v4().to_string();
            navigator.push(&Route::Play { room_id });
        })
    };

    let on_room_id_input = {
        let room_id_input = room_id_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let value = input.value();
            // Extract room ID from URL if pasted
            let extracted = extract_room_id(&value);
            room_id_input.set(extracted);
        })
    };

    let on_join_room = {
        let navigator = navigator.clone();
        let room_id_input = room_id_input.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let room_id = (*room_id_input).trim().to_string();
            if !room_id.is_empty() {
                navigator.push(&Route::Play { room_id });
            }
        })
    };

    html! {
        <>
            { if *show_welcome_modal {
                html! { <WelcomeModal on_complete={on_welcome_complete} /> }
            } else {
                html! {}
            }}

            <Layout>
                <div class="home-content">
                    <div class="home-hero">
                        <h1 class="home-title">{ "Marble Live" }</h1>
                        <p class="home-subtitle">{ "Real-time multiplayer marble racing" }</p>
                    </div>

                    <div class="home-actions">
                        <button class="btn btn-primary btn-large" onclick={on_start_race}>
                            { "Start Race" }
                        </button>

                        <div class="divider">
                            <span>{ "or" }</span>
                        </div>

                        <form class="join-form" onsubmit={on_join_room}>
                            <input
                                type="text"
                                class="room-id-input"
                                placeholder="Enter Room ID or URL"
                                value={(*room_id_input).clone()}
                                oninput={on_room_id_input}
                            />
                            <button
                                type="submit"
                                class="btn btn-secondary"
                                disabled={room_id_input.is_empty()}
                            >
                                { "Join Room" }
                            </button>
                        </form>
                    </div>
                </div>
            </Layout>
        </>
    }
}
