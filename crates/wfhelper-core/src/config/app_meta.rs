//! App identity (original: `config/shared/appMeta.ts`, `config/runtime/appIdentity.ts`).

pub const APP_NAME: &str = "WFHelper";
pub const APP_ID: &str = "wfhelper-rust";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO_URL: &str = "https://github.com/WFHelper/WFHelper";

/// User-Agent for every outgoing request. Generic on purpose: no account data.
pub fn user_agent() -> String {
    format!("{APP_ID}/{APP_VERSION}")
}
