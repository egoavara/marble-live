//! Home page with lobby functionality.

use std::cell::RefCell;
use std::rc::Rc;

use crate::components::{Layout, WelcomeModal};
use crate::hooks::{use_config_secret, use_config_username, use_grpc_room_service};
use crate::routes::Route;
use crate::util::async_callback;
use marble_proto::room::{CreateRoomRequest, PlayerAuth};
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
    let username = use_config_username();
    let secret = use_config_secret();

    let show_welcome_modal = username.is_none();

    let room_service = use_grpc_room_service();

    let room_id_input = use_state(|| "".to_string());
    let on_room_id_input = {
        let room_id_input = room_id_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            room_id_input.set(input.value());
        })
    };

    let on_join_room = {
        let room_id_input = room_id_input.clone();
        let navigator = navigator.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let extracted_room_id = extract_room_id(&room_id_input);
            if !extracted_room_id.is_empty() {
                // navigator.push(&Route::Play {
                //     room_id: extracted_room_id,
                // });
            }
        })
    };

    let on_start_race = {
        async_callback(
            (
                navigator.clone(),
                room_service.clone(),
                username.clone(),
                secret.clone(),
            ),
            async move |(navigator, room_service, username, secret)| {
                match room_service
                    .borrow_mut()
                    .create_room(CreateRoomRequest {
                        host: Some(PlayerAuth {
                            id: username
                                .as_ref()
                                .cloned()
                                .expect("Username must be set to create a room"),
                            secret: secret.to_string(),
                        }),
                        max_players: 8,
                    })
                    .await
                {
                    Ok(response) => {
                        let new_room_id = response.into_inner().room_id;
                        navigator.push(&Route::Play {
                            room_id: new_room_id,
                        });
                    }
                    Err(err) => {
                        // TODO: 사용자 이름이 없거나 매개변수에 문제가 있는 경우에 대한 처리 추가
                        tracing::error!("Failed to create room: {}", err);
                    }
                }

                // navigator.push(&Route::Play {
                //     room_id: new_room_id,
                // });
            },
        )
    };

    html! {
        <>
            { if show_welcome_modal {
                html! { <WelcomeModal /> }
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
