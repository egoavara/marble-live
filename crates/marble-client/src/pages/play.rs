//! Play page with Bevy-based marble game.

use yew::prelude::*;

use crate::components::{GameView, Layout, RoomState, use_room_service};

/// Props for the PlayPage component.
#[derive(Properties, PartialEq)]
pub struct PlayPageProps {
    pub room_id: String,
}

/// Play page component with Bevy game.
#[function_component(PlayPage)]
pub fn play_page(props: &PlayPageProps) -> Html {
    let room_id = props.room_id.clone();
    let room_service = use_room_service();

    // On mount: join room. On unmount: leave.
    {
        let rs = room_service.clone();
        let room_id = room_id.clone();
        use_effect_with(room_id.clone(), move |room_id| {
            rs.join(room_id);
            let rs = rs.clone();
            move || rs.leave()
        });
    }

    let content = match room_service.room_state() {
        RoomState::Idle | RoomState::Joining { .. } => {
            html! {
                <div class="connecting-overlay fullscreen">
                    <div class="connecting-spinner"></div>
                    <p>{"Joining room..."}</p>
                    <p class="room-id">{&room_id}</p>
                </div>
            }
        }
        RoomState::Error { message, .. } => {
            html! {
                <div class="error-overlay fullscreen">
                    <p class="error-message">{"Failed to join room"}</p>
                    <p class="error-detail">{message}</p>
                </div>
            }
        }
        RoomState::Active {
            signaling_url,
            is_host,
            ..
        } => {
            html! {
                <GameView
                    room_id={room_id.clone()}
                    signaling_url={signaling_url}
                    is_host={is_host}
                />
            }
        }
    };

    html! {
        <Layout transparent={true}>
            <div class="game-fullscreen">
                {content}
            </div>
        </Layout>
    }
}
