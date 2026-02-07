use yew::Callback;

pub fn async_callback<Deps, F, Fut, E>(deps: Deps, f: F) -> Callback<E>
where
    Deps: Clone + 'static,
    F: Fn(Deps) -> Fut + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    Callback::from(move |_| {
        let deps = deps.clone();
        wasm_bindgen_futures::spawn_local(f(deps));
    })
}
