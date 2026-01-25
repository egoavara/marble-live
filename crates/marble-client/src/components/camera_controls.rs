//! Camera mode control buttons.
//!
//! Displays Q/W/E buttons at top-center of screen.
//! - Q: FollowMe (my marble)
//! - W: FollowLeader (1st place)
//! - E: Overview (entire map)

use std::cell::RefCell;
use std::rc::Rc;

use yew::prelude::*;

use crate::camera::{CameraMode, CameraState};

/// Props for CameraControls component.
#[derive(Properties)]
pub struct CameraControlsProps {
    /// Reference to the camera state.
    pub camera_state: Rc<RefCell<CameraState>>,
    /// Current camera mode (for re-render on change).
    pub current_mode: CameraMode,
    /// Callback when mode changes (for localStorage sync).
    pub on_mode_change: Callback<CameraMode>,
}

impl PartialEq for CameraControlsProps {
    fn eq(&self, other: &Self) -> bool {
        // Compare by pointer for Rc, by value for others
        Rc::ptr_eq(&self.camera_state, &other.camera_state)
            && self.current_mode == other.current_mode
            && self.on_mode_change == other.on_mode_change
    }
}

/// Camera control buttons component.
#[function_component(CameraControls)]
pub fn camera_controls(props: &CameraControlsProps) -> Html {
    let current_mode = props.current_mode;

    let on_click_follow_me = {
        let camera_state = props.camera_state.clone();
        let on_mode_change = props.on_mode_change.clone();
        Callback::from(move |_: MouseEvent| {
            camera_state.borrow_mut().set_mode(CameraMode::FollowMe);
            on_mode_change.emit(CameraMode::FollowMe);
        })
    };

    let on_click_follow_leader = {
        let camera_state = props.camera_state.clone();
        let on_mode_change = props.on_mode_change.clone();
        Callback::from(move |_: MouseEvent| {
            camera_state.borrow_mut().set_mode(CameraMode::FollowLeader);
            on_mode_change.emit(CameraMode::FollowLeader);
        })
    };

    let on_click_overview = {
        let camera_state = props.camera_state.clone();
        let on_mode_change = props.on_mode_change.clone();
        Callback::from(move |_: MouseEvent| {
            camera_state.borrow_mut().set_mode(CameraMode::Overview);
            on_mode_change.emit(CameraMode::Overview);
        })
    };

    // Lucide-style SVG icons
    let icon_user = html! {
        // User icon (FollowMe)
        <svg class="camera-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="8" r="5"/>
            <path d="M20 21a8 8 0 0 0-16 0"/>
        </svg>
    };

    let icon_crown = html! {
        // Crown icon (FollowLeader)
        <svg class="camera-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M11.562 3.266a.5.5 0 0 1 .876 0L15.39 8.87a1 1 0 0 0 1.516.294L21.183 5.5a.5.5 0 0 1 .798.519l-2.834 10.246a1 1 0 0 1-.956.734H5.81a1 1 0 0 1-.957-.734L2.02 6.02a.5.5 0 0 1 .798-.519l4.276 3.664a1 1 0 0 0 1.516-.294z"/>
            <path d="M5 21h14"/>
        </svg>
    };

    let icon_maximize = html! {
        // Maximize/Grid icon (Overview)
        <svg class="camera-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 3H5a2 2 0 0 0-2 2v3"/>
            <path d="M21 8V5a2 2 0 0 0-2-2h-3"/>
            <path d="M3 16v3a2 2 0 0 0 2 2h3"/>
            <path d="M16 21h3a2 2 0 0 0 2-2v-3"/>
        </svg>
    };

    html! {
        <div class="camera-controls">
            <button
                class={classes!(
                    "camera-btn",
                    (current_mode == CameraMode::FollowMe).then_some("active")
                )}
                onclick={on_click_follow_me}
                title="Q - Follow Me"
            >
                { icon_user }
            </button>
            <button
                class={classes!(
                    "camera-btn",
                    (current_mode == CameraMode::FollowLeader).then_some("active")
                )}
                onclick={on_click_follow_leader}
                title="W - Follow Leader"
            >
                { icon_crown }
            </button>
            <button
                class={classes!(
                    "camera-btn",
                    (current_mode == CameraMode::Overview).then_some("active")
                )}
                onclick={on_click_overview}
                title="E - Overview"
            >
                { icon_maximize }
            </button>
        </div>
    }
}
