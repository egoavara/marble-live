use std::ops::Deref;

use yew::prelude::*;

#[hook]
pub fn use_querystring<T, F>(key: &'static str, init_fn: F) -> UseStateHandle<T>
where
    T: 'static + Clone + serde::Serialize + serde::de::DeserializeOwned + PartialEq,
    F: Fn() -> T + 'static,
{
    let state = use_state(|| {
        let window = web_sys::window().expect("no window");
        let location = window.location();
        let search = location.search().unwrap_or_default();

        let params = web_sys::UrlSearchParams::new_with_str(&search).ok();

        if let Some(params) = params {
            if let Some(value) = params.get(key) {
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
            let window = web_sys::window().expect("no window");
            let location = window.location();
            let search = location.search().unwrap_or_default();

            let params = web_sys::UrlSearchParams::new_with_str(&search)
                .unwrap_or_else(|_| web_sys::UrlSearchParams::new().unwrap());

            if let Ok(serialized) = serde_json::to_string(&state.deref()) {
                params.set(key, &serialized);
            }

            let new_url = {
                let pathname = location.pathname().unwrap_or_default();
                let params_str = params.to_string().as_string().unwrap_or_default();
                let hash = location.hash().unwrap_or_default();

                if params_str.is_empty() {
                    format!("{}{}", pathname, hash)
                } else {
                    format!("{}?{}{}", pathname, params_str, hash)
                }
            };

            let history = window.history().expect("no history");
            let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&new_url));

            || ()
        });
    }

    state
}
