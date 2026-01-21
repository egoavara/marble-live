//! Marble-Live Client
//!
//! Yew WASM frontend application.

use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    html! {
        <main>
            <h1>{ "Marble Live" }</h1>
            <p>{ "WebRTC P2P 구슬 룰렛 게임" }</p>
        </main>
    }
}

fn main() {
    wasm_bindgen_futures::spawn_local(async {
        yew::Renderer::<App>::new().render();
    });
}
