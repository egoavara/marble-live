use yew::prelude::*;
use yew_router::prelude::*;

use crate::routes::Route;

#[derive(Properties, PartialEq)]
pub struct LogoProps {
    #[prop_or(false)]
    pub state: bool, // true일 때 애니메이션 재생
    #[prop_or(32)]
    pub size: u32, // SVG 크기 (px)
    #[prop_or(true)]
    pub link: bool, // true일 때 클릭하면 홈으로 이동
}

#[function_component(Logo)]
pub fn logo(props: &LogoProps) -> Html {
    let class = classes!(
        "logo-marble",
        props.state.then_some("logo-marble-animating")
    );

    let content = html! {
        <>
            <div class="speed-line line-1"></div>
            <div class="speed-line line-2"></div>
            <div class="speed-line line-3"></div>

            <svg class="marble-svg" viewBox="0 0 40 40"
                 width={props.size.to_string()} height={props.size.to_string()}>
                <circle cx="20" cy="20" r="16" />
                <path d="M12 12C15 15 25 15 28 12" opacity="0.4" />
                <path d="M10 20C15 25 25 25 30 20" stroke-width="1.5" />
                <circle cx="28" cy="12" r="1.2" fill="white" stroke="none" />
            </svg>
        </>
    };

    if props.link {
        html! {
            <Link<Route> to={Route::Home} classes={class}>
                { content }
            </Link<Route>>
        }
    } else {
        html! {
            <div class={class}>
                { content }
            </div>
        }
    }
}
