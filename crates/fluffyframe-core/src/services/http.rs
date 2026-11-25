//! Shared blocking HTTP client (original: `config/shared/fetchWithTimeout.ts`).

use std::sync::OnceLock;

use anyhow::{Context, bail};
use reqwest::blocking::Client;

use crate::config::{app_meta, constants};

pub fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(app_meta::user_agent())
            .timeout(constants::HTTP_TIMEOUT)
            .gzip(true)
            .build()
            .expect("HTTP client")
    })
}

pub fn get_bytes(url: &str) -> crate::Result<Vec<u8>> {
    let resp = client().get(url).send().with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        bail!("HTTP {} for {url}", resp.status());
    }
    Ok(resp.bytes()?.to_vec())
}

pub fn get_text(url: &str) -> crate::Result<String> {
    let bytes = get_bytes(url)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Removes control characters DE leaves in its JSON (keeps \t \n \r).
pub fn strip_control_chars(text: &str) -> String {
    text.chars()
        .filter(|&c| !(c < '\u{20}' && c != '\t' && c != '\n' && c != '\r'))
        .collect()
}
