//! GitHub without the CLI.
//!
//! The token lives in the OS keychain under this app's service name, and every
//! call is a plain HTTPS request. Nothing here reads or writes the machine's
//! `gh` configuration, and nothing changes which account any other tool sees —
//! the failure mode that made earlier attempts destructive.

#[cfg(feature = "keyring")]
use canonwright_governance::secrets::{KeychainSecretStore, Secret, SecretStore};

/// Keychain service name. One entry per account would be `github:<user>`; the
/// default entry is the account this app is signed in as.
const SERVICE: &str = "canonwright";
const DEFAULT_ENTRY: &str = "github.default";
const USER_AGENT: &str = "canonwright";
const API: &str = "https://api.github.com";

#[cfg(feature = "keyring")]
fn store() -> KeychainSecretStore {
    KeychainSecretStore::new(SERVICE)
}

/// The stored token, if the user has signed in.
pub fn stored_token() -> Option<String> {
    #[cfg(not(feature = "keyring"))]
    {
        return None;
    }
    #[cfg(feature = "keyring")]
    match store().get(DEFAULT_ENTRY) {
        Ok(Some(secret)) => Some(secret.expose().to_owned()),
        _ => None,
    }
}

pub fn store_token(token: &str) -> Result<(), String> {
    #[cfg(not(feature = "keyring"))]
    {
        let _ = token;
        return Err(
            "this build has no keychain support; rebuild with the keyring feature".to_owned(),
        );
    }
    #[cfg(feature = "keyring")]
    store()
        .set(DEFAULT_ENTRY, &Secret::new(token.trim()))
        .map_err(|error| format!("could not save the token in the keychain: {error}"))
}

pub fn forget_token() -> Result<(), String> {
    #[cfg(not(feature = "keyring"))]
    {
        return Ok(());
    }
    #[cfg(feature = "keyring")]
    store()
        .delete(DEFAULT_ENTRY)
        .map_err(|error| format!("could not remove the stored token: {error}"))
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("could not build an HTTP client: {error}"))
}

/// `GET /user` — the account behind a token, or why it was rejected.
pub fn whoami(token: &str) -> Result<String, String> {
    let response = client()?
        .get(format!("{API}/user"))
        .bearer_auth(token)
        .send()
        .map_err(|error| format!("could not reach GitHub: {error}"))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("GitHub rejected the token".to_owned());
    }
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status()));
    }
    let body: serde_json::Value = response
        .json()
        .map_err(|error| format!("GitHub sent something unexpected: {error}"))?;
    body.get("login")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "GitHub answered without a login".to_owned())
}

/// `GET /user/repos` — the repositories this token can write to, newest first.
pub fn repos(token: &str) -> Result<Vec<String>, String> {
    let response = client()?
        .get(format!("{API}/user/repos"))
        .query(&[
            ("per_page", "100"),
            ("sort", "updated"),
            ("affiliation", "owner,collaborator"),
        ])
        .bearer_auth(token)
        .send()
        .map_err(|error| format!("could not reach GitHub: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status()));
    }
    let body: serde_json::Value = response
        .json()
        .map_err(|error| format!("GitHub sent something unexpected: {error}"))?;
    let items = body
        .as_array()
        .ok_or_else(|| "GitHub answered with a list, not a page".to_owned())?;
    Ok(items
        .iter()
        .filter_map(|item| item.get("full_name").and_then(|value| value.as_str()))
        .map(str::to_owned)
        .collect())
}
