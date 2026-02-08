//! Play page with Bevy-based marble game.

use yew::prelude::*;

use crate::components::{GameView, Layout, RoomState, use_room_service};
use crate::hooks::PlayerInfo;

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

    // Game end callback (for winner modal)
    let game_ended = use_state(|| false);
    let winners = use_state(Vec::<PlayerInfo>::new);

    let _on_game_end = {
        let game_ended = game_ended.clone();
        let winners = winners.clone();
        Callback::from(move |players: Vec<PlayerInfo>| {
            winners.set(players);
            game_ended.set(true);
        })
    };

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
                <>
                    <GameView
                        room_id={room_id.clone()}
                        signaling_url={signaling_url}
                        is_host={is_host}
                    />

                    // Winner modal
                    if *game_ended {
                        <WinnerOverlay
                            winners={(*winners).clone()}
                            on_close={Callback::from({
                                let game_ended = game_ended.clone();
                                move |_| game_ended.set(false)
                            })}
                        />
                    }
                </>
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

/// Props for winner overlay.
#[derive(Properties, PartialEq)]
struct WinnerOverlayProps {
    winners: Vec<PlayerInfo>,
    on_close: Callback<()>,
}

/// Winner overlay component.
#[function_component(WinnerOverlay)]
fn winner_overlay(props: &WinnerOverlayProps) -> Html {
    let on_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    html! {
        <div class="winner-overlay" onclick={on_click}>
            <div class="winner-modal">
                <h2>{"Game Over!"}</h2>
                <div class="winner-list">
                    { for props.winners.iter().enumerate().map(|(i, player)| {
                        let color = format!(
                            "rgb({}, {}, {})",
                            player.color[0], player.color[1], player.color[2]
                        );
                        let medal = match i {
                            0 => "🥇",
                            1 => "🥈",
                            2 => "🥉",
                            _ => "",
                        };
                        html! {
                            <div class="winner-item">
                                <span class="winner-medal">{medal}</span>
                                <span
                                    class="winner-color"
                                    style={format!("background-color: {}", color)}
                                />
                                <span class="winner-name">{&player.name}</span>
                                <span class="winner-rank">{format!("#{}", i + 1)}</span>
                            </div>
                        }
                    })}
                </div>
                <p class="winner-hint">{"Click anywhere to close"}</p>
            </div>
        </div>
    }
}
