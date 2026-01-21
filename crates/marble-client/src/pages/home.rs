//! Home page (placeholder).

use yew::prelude::*;

/// Home page component - placeholder for future main game.
#[function_component(HomePage)]
pub fn home_page() -> Html {
    html! {
        <main class="page home-page">
            <h1>{ "Marble Live" }</h1>
            <p>{ "Coming soon..." }</p>
        </main>
    }
}
