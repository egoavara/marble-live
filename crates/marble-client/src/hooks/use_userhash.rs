//! Hook for loading and applying user settings.

use yew::prelude::*;

use crate::{
    fingerprint,
    hooks::{use_fingerprint, use_localstorage},
    util::generate_hash,
};

const CONFIG_NAME_KEY: &str = "$marble-live$/config/name";

#[hook]
pub fn use_userhash(data: UseStateHandle<String>) -> UseStateHandle<String> {
    let userhash = use_state(String::new);
    let fingerprint = use_fingerprint();
    {
        let userhash = userhash.clone();
        let data = data.clone();
        use_effect_with(data.clone(), move |data| {
            let data = (*data).clone();
            userhash.set(make_userhash(&data, &fingerprint));
            ()
        });
    }
    userhash
}
#[hook]
pub fn use_opt_userhash(data: UseStateHandle<Option<String>>) -> UseStateHandle<Option<String>> {
    let userhash = use_state(|| None);
    let fingerprint = use_fingerprint();
    tracing::info!(fingerprint = ?*fingerprint, "use_opt_userhash fingerprint");
    {
        let userhash = userhash.clone();
        let data = data.clone();
        use_effect_with(
            (data.clone(), fingerprint.clone()),
            move |(data, fingerprint)| {
                match &**data {
                    Some(data) => {
                        userhash.set(Some(make_userhash(data, &fingerprint)));
                    }
                    None => {
                        userhash.set(None);
                    }
                }
                ()
            },
        );
    }
    userhash
}

pub fn make_userhash(name: &str, fingerprint: &Option<String>) -> String {
    let Some(fingerprint) = fingerprint else {
        return String::new();
    };
    let combined = format!("{}{}", fingerprint, name);
    let mut hash: u32 = 0;
    for byte in combined.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u32::from(byte));
    }

    // Return 4-digit hex code
    format!("{:04X}", hash & 0xFFFF)
}
