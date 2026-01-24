//! Hook for game tick and rendering loop.

use crate::p2p::protocol::P2PMessage;
use crate::p2p::state::{P2PAction, P2PPhase, P2PStateContext};
use crate::p2p::sync::{SyncTracker, HASH_EXCHANGE_INTERVAL};
use crate::renderer::CanvasRenderer;
use gloo::timers::callback::Interval;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

/// Canvas dimensions.
pub const CANVAS_WIDTH: u32 = 800;
pub const CANVAS_HEIGHT: u32 = 600;

/// Manage game tick and rendering loop.
///
/// This hook handles:
/// - Canvas initialization
/// - Game tick during Countdown/Running phases
/// - Frame hash exchange for sync verification
/// - Rendering the game state
#[hook]
pub fn use_game_loop(
    canvas_ref: &NodeRef,
    state: &P2PStateContext,
    renderer_ref: &Rc<RefCell<Option<CanvasRenderer>>>,
    sync_tracker: &Rc<RefCell<SyncTracker>>,
) {
    // Initialize canvas and renderer
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();
        let phase = state.phase.clone();

        use_effect_with(phase.clone(), move |_phase| {
            if renderer_ref.borrow().is_none() {
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    canvas.set_width(CANVAS_WIDTH);
                    canvas.set_height(CANVAS_HEIGHT);

                    if let Ok(renderer) = CanvasRenderer::new(&canvas) {
                        *renderer_ref.borrow_mut() = Some(renderer);
                    }
                }
            }
            || ()
        });
    }

    // Game tick effect
    {
        let state = state.clone();
        let sync_tracker = sync_tracker.clone();

        use_effect_with(state.phase.clone(), move |phase| {
            let should_tick = matches!(
                phase,
                P2PPhase::Countdown { .. } | P2PPhase::Running
            );

            let interval: Option<Interval> = if !should_tick {
                None
            } else {
                let state_inner = state.clone();
                let sync_tracker_inner = sync_tracker.clone();

                Some(Interval::new(16, move || {
                    state_inner.dispatch(P2PAction::Tick);

                    let frame = state_inner.game_state.current_frame();
                    if frame > 0 && frame % HASH_EXCHANGE_INTERVAL == 0 {
                        let hash = state_inner.game_state.compute_hash();
                        let msg = P2PMessage::FrameHash { frame, hash };
                        state_inner.network.borrow_mut().broadcast(&msg.encode());
                        sync_tracker_inner.borrow_mut().mark_hash_sent(frame);
                    }
                }))
            };

            move || drop(interval)
        });
    }

    // Render effect
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();
        let game_state = state.game_state.clone();
        let phase = state.phase.clone();

        use_effect(move || {
            if !matches!(phase, P2PPhase::Disconnected | P2PPhase::Connecting) {
                // Ensure renderer is initialized
                if renderer_ref.borrow().is_none() {
                    if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                        canvas.set_width(CANVAS_WIDTH);
                        canvas.set_height(CANVAS_HEIGHT);

                        if let Ok(renderer) = CanvasRenderer::new(&canvas) {
                            *renderer_ref.borrow_mut() = Some(renderer);
                        }
                    }
                }

                // Render the game
                if let Some(renderer) = renderer_ref.borrow().as_ref() {
                    renderer.render(&game_state);
                }
            }
            || ()
        });
    }
}
