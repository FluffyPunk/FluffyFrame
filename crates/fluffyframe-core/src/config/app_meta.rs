//! App identity (original: `config/shared/appMeta.ts`, `config/runtime/appIdentity.ts`).

pub const APP_NAME: &str = "FluffyFrame";
pub const APP_ID: &str = "fluffyframe";
/// The app's id with the desktop portals (reverse DNS of fluffypunk.fyi):
/// global shortcuts are listed under it (`fyi.fluffypunk.fluffyframe:check-riven`).
/// Desktops keep their bindings under it, so it must not change.
pub const PORTAL_APP_ID: &str = "fyi.fluffypunk.fluffyframe";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO_URL: &str = "https://github.com/FluffyPunk/FluffyFrame";
/// WFHelper, the Electron app FluffyFrame is a port of.
pub const ORIGINAL_REPO_URL: &str = "https://github.com/WFHelper/WFHelper";

/// User-Agent for every outgoing request. Generic on purpose: no account data.
pub fn user_agent() -> String {
    format!("{APP_ID}/{APP_VERSION}")
}

/// `FLUFFYFRAME_<name>`, falling back to the pre-rename `WFHELPER_<name>`.
pub fn env_var(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(format!("FLUFFYFRAME_{name}")).or_else(|| std::env::var_os(format!("WFHELPER_{name}")))
}
