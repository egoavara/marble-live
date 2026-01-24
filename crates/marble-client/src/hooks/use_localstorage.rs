use std::ops::Deref;

use yew::prelude::*;

#[hook]
pub fn use_localstorage<T, F>(key: &'static str, init_fn: F) -> UseStateHandle<T>
where
    T: 'static + Clone + serde::Serialize + serde::de::DeserializeOwned + PartialEq,
    F: Fn() -> T + 'static,
{
    let state = use_state(|| {
        let storage = web_sys::window().and_then(|win| win.local_storage().ok().flatten());

        if let Some(storage) = storage {
            if let Ok(Some(value)) = storage.get_item(key) {
                if let Ok(deserialized) = serde_json::from_str::<T>(&value) {
                    return deserialized;
                }
            }
        }
        init_fn()
    });
    {
        let state = state.clone();
        use_effect_with(state.clone(), move |state| {
            let storage = web_sys::window().and_then(|win| win.local_storage().ok().flatten());
            if let Some(storage) = storage {
                if let Ok(serialized) = serde_json::to_string(&state.deref()) {
                    let _ = storage.set_item(key, &serialized);
                }
            }
            || ()
        });
    }
    state
}
