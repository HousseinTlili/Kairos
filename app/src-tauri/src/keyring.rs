//! Hermes API key storage in Windows Credential Manager (M1 §3).
//!
//! The credential's target name is `hermes-api-key.kairos` (keyring's default `<user>.<service>`).
//! This module shadows the `keyring` crate inside this crate; the crate is `::keyring`.

use crate::hermes::client::ApiKey;

const SERVICE: &str = "kairos";
const USER: &str = "hermes-api-key";

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("Windows Credential Manager: {0}")]
pub struct KeyStoreError(String);

// keyring errors describe the store and the entry, never the secret, so they're safe to show.
fn store_error(err: ::keyring::Error) -> KeyStoreError {
    KeyStoreError(err.to_string())
}

fn entry() -> Result<::keyring::Entry, KeyStoreError> {
    ::keyring::Entry::new(SERVICE, USER).map_err(store_error)
}

/// The stored key, or `None` before first launch.
pub fn load() -> Result<Option<ApiKey>, KeyStoreError> {
    match entry()?.get_password() {
        Ok(key) => Ok(Some(ApiKey::new(key))),
        Err(::keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(store_error(err)),
    }
}

pub fn save(key: &ApiKey) -> Result<(), KeyStoreError> {
    entry()?.set_password(key.expose()).map_err(store_error)
}
