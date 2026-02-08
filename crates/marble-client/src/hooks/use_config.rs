//! Hook for loading and applying user settings.

use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use yew::prelude::*;

use crate::{
    fingerprint,
    hooks::{use_fingerprint, use_localstorage, use_userhash::make_userhash},
    util::generate_hash,
};

const CONFIG_USERNAME_KEY: &str = "$marble-live$/config/username";
const CONFIG_SECRET_KEY: &str = "$marble-live$/config/secret";
const CONFIG_AUTH_TOKEN_KEY: &str = "$marble-live$/config/auth-token";

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum ConfigSecret {
    Anonymous(String),
}
impl ConfigSecret {
    pub fn to_string(&self) -> String {
        match self {
            ConfigSecret::Anonymous(s) => format!("anon:{}", s),
        }
    }
}

#[hook]
pub fn use_config_username() -> UseStateHandle<Option<String>> {
    let name_storage = use_localstorage(CONFIG_USERNAME_KEY, || None::<String>);
    name_storage
}

#[hook]
pub fn use_auth_token() -> UseStateHandle<Option<String>> {
    use_localstorage(CONFIG_AUTH_TOKEN_KEY, || None::<String>)
}

#[hook]
pub fn use_config_secret() -> UseStateHandle<ConfigSecret> {
    let secret_storage = use_localstorage(CONFIG_SECRET_KEY, || {
        let mut rng = rand::rng();

        let anon_hashcode: String = (0..40).map(|_| rng.sample(Alphanumeric) as char).collect();
        ConfigSecret::Anonymous(anon_hashcode)
    });
    secret_storage
}
