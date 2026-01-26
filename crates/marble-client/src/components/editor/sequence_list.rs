//! Keyframe sequence list component.

use marble_core::map::KeyframeSequence;
use yew::prelude::*;

/// Props for the SequenceList component.
#[derive(Properties, PartialEq)]
pub struct SequenceListProps {
    pub sequences: Vec<KeyframeSequence>,
    pub selected_index: Option<usize>,
    pub on_select: Callback<Option<usize>>,
    pub on_add: Callback<KeyframeSequence>,
    pub on_delete: Callback<usize>,
    pub on_update: Callback<(usize, KeyframeSequence)>,
}

/// Sequence list component for displaying and managing keyframe sequences.
#[function_component(SequenceList)]
pub fn sequence_list(props: &SequenceListProps) -> Html {
    let show_add_modal = use_state(|| false);
    let new_sequence_name = use_state(|| String::new());

    let toggle_add_modal = {
        let show_add_modal = show_add_modal.clone();
        let new_sequence_name = new_sequence_name.clone();
        Callback::from(move |_: MouseEvent| {
            if *show_add_modal {
                new_sequence_name.set(String::new());
            }
            show_add_modal.set(!*show_add_modal);
        })
    };

    let on_name_input = {
        let new_sequence_name = new_sequence_name.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                new_sequence_name.set(input.value());
            }
        })
    };

    let add_sequence = {
        let on_add = props.on_add.clone();
        let show_add_modal = show_add_modal.clone();
        let new_sequence_name = new_sequence_name.clone();
        Callback::from(move |_: MouseEvent| {
            let name = (*new_sequence_name).clone();
            if !name.is_empty() {
                on_add.emit(KeyframeSequence {
                    name,
                    target_ids: vec![],
                    keyframes: vec![],
                    autoplay: true,
                });
                new_sequence_name.set(String::new());
                show_add_modal.set(false);
            }
        })
    };

    html! {
        <div class="sequence-list">
            <div class="sequence-list-items">
                {for props.sequences.iter().enumerate().map(|(i, seq)| {
                    let on_select = props.on_select.clone();
                    let on_delete = props.on_delete.clone();
                    let on_update = props.on_update.clone();
                    let is_selected = props.selected_index == Some(i);
                    let sequence = seq.clone();

                    let on_item_click = {
                        let on_select = on_select.clone();
                        Callback::from(move |_: MouseEvent| {
                            on_select.emit(Some(i));
                        })
                    };

                    let on_delete_click = {
                        let on_delete = on_delete.clone();
                        Callback::from(move |e: MouseEvent| {
                            e.stop_propagation();
                            on_delete.emit(i);
                        })
                    };

                    let on_autoplay_toggle = {
                        let on_update = on_update.clone();
                        let seq_clone = sequence.clone();
                        Callback::from(move |e: MouseEvent| {
                            e.stop_propagation();
                            let mut updated = seq_clone.clone();
                            updated.autoplay = !updated.autoplay;
                            on_update.emit((i, updated));
                        })
                    };

                    let target_summary = if seq.target_ids.is_empty() {
                        "(no targets)".to_string()
                    } else if seq.target_ids.len() == 1 {
                        seq.target_ids[0].clone()
                    } else {
                        format!("{}, +{}", seq.target_ids[0], seq.target_ids.len() - 1)
                    };

                    html! {
                        <div
                            class={classes!("sequence-list-item", is_selected.then_some("selected"))}
                            onclick={on_item_click}
                        >
                            <div class="sequence-item-header">
                                <span class="sequence-name">{&seq.name}</span>
                                <div class="sequence-item-actions">
                                    <button
                                        class={classes!("sequence-autoplay-btn", seq.autoplay.then_some("active"))}
                                        onclick={on_autoplay_toggle}
                                        title={if seq.autoplay { "Autoplay On" } else { "Autoplay Off" }}
                                    >
                                        {if seq.autoplay { "A" } else { "-" }}
                                    </button>
                                    <button
                                        class="sequence-delete-btn"
                                        onclick={on_delete_click}
                                        title="Delete"
                                    >
                                        {"x"}
                                    </button>
                                </div>
                            </div>
                            <div class="sequence-item-info">
                                <span class="sequence-targets">{target_summary}</span>
                                <span class="sequence-keyframe-count">
                                    {format!("{} keyframes", seq.keyframes.len())}
                                </span>
                            </div>
                        </div>
                    }
                })}
                if props.sequences.is_empty() {
                    <div class="sequence-list-empty">
                        {"No sequences. Click + to add."}
                    </div>
                }
            </div>
            <div class="sequence-list-footer">
                <button class="add-btn" onclick={toggle_add_modal.clone()}>
                    {"+ Add Sequence"}
                </button>
            </div>
            if *show_add_modal {
                <div class="sequence-add-modal">
                    <div class="sequence-add-modal-content">
                        <div class="sequence-add-modal-header">
                            {"New Sequence"}
                        </div>
                        <input
                            type="text"
                            class="sequence-name-input"
                            placeholder="Sequence name"
                            value={(*new_sequence_name).clone()}
                            oninput={on_name_input}
                        />
                        <div class="sequence-add-modal-actions">
                            <button class="btn-cancel" onclick={toggle_add_modal}>
                                {"Cancel"}
                            </button>
                            <button
                                class="btn-confirm"
                                onclick={add_sequence}
                                disabled={new_sequence_name.is_empty()}
                            >
                                {"Add"}
                            </button>
                        </div>
                    </div>
                </div>
            }
        </div>
    }
}
