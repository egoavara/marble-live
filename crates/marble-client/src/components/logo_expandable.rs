use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::Logo;
use crate::routes::Route;

#[derive(Properties, PartialEq)]
pub struct LogoExpandableProps {
    #[prop_or(false)]
    pub state: bool, // true일 때 전체 애니메이션 재생
    #[prop_or(32)]
    pub size: u32,
}

#[function_component(LogoExpandable)]
pub fn logo_expandable(props: &LogoExpandableProps) -> Html {
    let class = classes!(
        "logo-expandable",
        props.state.then_some("logo-expandable-animating")
    );

    html! {
        <Link<Route> to={Route::Home} classes={class}>
            <Logo state={props.state} size={props.size} link={false} />
            <span class="logo-text">
                { "Marble " }
                <span class="logo-text-accent">{ "Live" }</span>
            </span>
        </Link<Route>>
    }
}
