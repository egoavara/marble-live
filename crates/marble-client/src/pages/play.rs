//! Play page with P2P multiplayer game.

use crate::components::{GameView, Layout};
use crate::hooks::{use_join_room, JoinRoomState};
use yew::prelude::*;

/// Props for the PlayPage component.
#[derive(Properties, PartialEq)]
pub struct PlayPageProps {
    pub room_id: String,
}

/// Play page component with P2P multiplayer.
#[function_component(PlayPage)]
pub fn play_page(props: &PlayPageProps) -> Html {
    let room_id = props.room_id.clone();
    let join_state = use_join_room(&room_id);

    let content = match &*join_state {
        JoinRoomState::Idle | JoinRoomState::Joining => {
            html! {
                <div class="connecting-overlay fullscreen">
                    <div class="connecting-spinner"></div>
                    <p>{"Joining room..."}</p>
                    <p class="room-id">{&room_id}</p>
                </div>
            }
        }
        JoinRoomState::Error(error) => {
            html! {
                <div class="error-overlay fullscreen">
                    <p class="error-message">{"Failed to join room"}</p>
                    <p class="error-detail">{error}</p>
                </div>
            }
        }
        JoinRoomState::Joined { signaling_url } => {
            html! {
                <GameView
                    room_id={room_id.clone()}
                    signaling_url={signaling_url.clone()}
                />
            }
        }
    };

    html! {
        <Layout>
            <div class="game-fullscreen">
                {content}
            </div>
        </Layout>
    }
}
