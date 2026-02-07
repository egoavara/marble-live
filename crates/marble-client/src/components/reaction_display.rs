//! ReactionDisplay component - floating emoji animations.

use gloo::timers::callback::Interval;
use yew::prelude::*;

use crate::hooks::Reaction;

/// Duration of the float animation in milliseconds
const ANIMATION_DURATION_MS: f64 = 3000.0;

/// Cleanup interval in milliseconds
const CLEANUP_INTERVAL_MS: u32 = 500;

/// Floating emoji state
#[derive(Clone)]
struct FloatingEmoji {
    id: u64,
    emoji: String,
    x_percent: f32,
    created_at: f64,
}

impl FloatingEmoji {
    fn is_expired(&self, now: f64) -> bool {
        now - self.created_at > ANIMATION_DURATION_MS
    }
}

/// Props for the ReactionDisplay component.
#[derive(Properties, PartialEq)]
pub struct ReactionDisplayProps {
    /// Reaction data from Bevy
    pub reactions: Vec<Reaction>,
}

/// ReactionDisplay component - shows floating emoji animations.
#[function_component(ReactionDisplay)]
pub fn reaction_display(props: &ReactionDisplayProps) -> Html {
    // 실제 데이터는 RefCell에 저장 (렌더링과 독립적으로 수정 가능)
    let emojis_ref = use_mut_ref(Vec::<FloatingEmoji>::new);
    let processed_ids = use_mut_ref(Vec::<u64>::new);

    // re-render를 트리거하기 위한 상태 (값 자체는 중요하지 않음)
    let render_version = use_state(|| 0u64);

    // 새 reaction 메시지 처리
    {
        let emojis_ref = emojis_ref.clone();
        let processed_ids = processed_ids.clone();
        let render_version = render_version.clone();
        let reactions = props.reactions.clone();

        use_effect_with(reactions.len(), move |_| {
            let now = js_sys::Date::now();
            let mut processed = processed_ids.borrow_mut();
            let mut emojis = emojis_ref.borrow_mut();
            let mut changed = false;

            for reaction in reactions.iter() {
                if processed.contains(&reaction.id) {
                    continue;
                }

                let x_percent = 10.0 + (js_sys::Math::random() as f32) * 80.0;

                emojis.push(FloatingEmoji {
                    id: reaction.id,
                    emoji: reaction.emoji.clone(),
                    x_percent,
                    created_at: now,
                });
                processed.push(reaction.id);
                changed = true;
            }

            // 만료된 이모지 정리
            let before_len = emojis.len();
            emojis.retain(|e| !e.is_expired(now));
            if emojis.len() != before_len {
                changed = true;
            }

            // borrow 해제 후 re-render 트리거
            drop(emojis);
            drop(processed);

            if changed {
                render_version.set(now as u64);
            }

            || ()
        });
    }

    // 주기적 정리 (interval)
    {
        let emojis_ref = emojis_ref.clone();
        let render_version = render_version.clone();

        use_effect_with((), move |_| {
            let interval = Interval::new(CLEANUP_INTERVAL_MS, move || {
                let now = js_sys::Date::now();
                let mut emojis = emojis_ref.borrow_mut();
                let before_len = emojis.len();

                emojis.retain(|e| !e.is_expired(now));

                if emojis.len() != before_len {
                    drop(emojis); // borrow 해제
                    render_version.set(now as u64);
                }
            });

            move || drop(interval)
        });
    }

    // 렌더링 - version을 읽어서 dependency 생성
    let _ = *render_version;

    // 현재 시점에서 유효한 이모지만 렌더링
    let now = js_sys::Date::now();
    let emojis = emojis_ref.borrow();
    let visible: Vec<_> = emojis.iter().filter(|e| !e.is_expired(now)).collect();

    html! {
        <div class="reaction-display">
            { for visible.iter().map(|emoji| {
                let style = format!("left: {}%;", emoji.x_percent);
                html! {
                    <span
                        class="floating-emoji"
                        style={style}
                        key={emoji.id.to_string()}
                    >
                        { &emoji.emoji }
                    </span>
                }
            }) }
        </div>
    }
}
