#![allow(dead_code)]

use color_eyre::{eyre::Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "dbrusty";

pub fn save_password(key: &str, password: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, key).context("Failed to create keyring entry")?;
    entry
        .set_password(password)
        .context("Failed to save password to keychain")?;
    Ok(())
}

pub fn get_password(key: &str) -> Result<Option<String>> {
    let entry = Entry::new(SERVICE_NAME, key).context("Failed to create keyring entry")?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e).context("Failed to read password from keychain"),
    }
}

pub fn delete_password(key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, key).context("Failed to create keyring entry")?;
    entry
        .delete_credential()
        .context("Failed to delete password from keychain")?;
    Ok(())
}
