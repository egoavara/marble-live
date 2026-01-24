//! Play page with P2P multiplayer game.

use crate::{components::Layout, renderer::CanvasRenderer};
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
    html! {
        <Layout>
            <div class="game-fullscreen">
                {&room_id}
            </div>
        </Layout>
    }
}
