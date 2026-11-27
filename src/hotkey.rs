//! The global "check riven" hotkey: reads the inventory at once for the roll
//! on the riven cycle screen, which EE.log reports only ~10 s late (the game
//! writes it in batches).
//!
//! - Windows, X11, macOS: the key in settings, registered with the system;
//!   changing the setting registers the new key.
//! - Wayland: the desktop portal's global shortcuts (Hyprland, KDE, GNOME 48+).
//!   The compositor owns the key: the setting is only the one asked for on
//!   first use; KDE and GNOME change it in their shortcut settings, Hyprland
//!   in its config. Hyprland doesn't tell the portal the key: it is looked up
//!   in Hyprland's own list of binds ([`Hotkey::desktop_key`]).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// The shortcut's id with the portal.
#[cfg(target_os = "linux")]
pub const PORTAL_ID: &str = "check-riven";

/// Where the hotkey stands, for settings.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Starting,
    /// Registered with the system under this key.
    Registered(String),
    /// Bound through the desktop portal: the key the desktop shows for it
    /// (empty: none bound yet). Wayland only; settings read it everywhere.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Portal(String),
    Failed(String),
}

/// Keeps the hotkey registered while alive.
pub struct Hotkey {
    status: Arc<Mutex<Status>>,
    /// Windows, X11, macOS; `None` through the portal or when the system
    /// refused a manager.
    native: Option<Mutex<Native>>,
    /// The last key looked up with the desktop, and when.
    desktop_key: Mutex<Option<(std::time::Instant, Option<String>)>>,
}

/// How long a key looked up with the desktop is reused (settings ask every frame).
const DESKTOP_KEY_FOR: std::time::Duration = std::time::Duration::from_secs(3);

struct Native {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
    /// The id the event handler answers to.
    id: Arc<AtomicU32>,
}

/// `combo` ("Ctrl+Alt+R") as a hotkey, or why it isn't one.
pub fn parse(combo: &str) -> Result<HotKey, String> {
    combo.parse::<HotKey>().map_err(|e| e.to_string())
}

/// A key pressed with `modifiers`, in settings' notation, when the system can
/// register it ("Ctrl+Shift+F5"); for recording a new hotkey in settings.
pub fn combo_of(modifiers: egui::Modifiers, key: egui::Key) -> Option<String> {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if modifiers.mac_cmd {
        parts.push("Super");
    }
    // egui's name first ("F5", "ArrowUp", "Space"), else its symbol ("[", "`").
    [key.name(), key.symbol_or_name()].into_iter().find_map(|name| {
        let combo = parts.iter().copied().chain([name]).collect::<Vec<_>>().join("+");
        parse(&combo).is_ok().then_some(combo)
    })
}

impl Hotkey {
    /// Registers `combo`; `on_press` runs (on another thread) each time it is
    /// pressed. Must be called on the UI thread: Windows and macOS deliver
    /// system hotkeys through its event loop. Failing only shows in
    /// [`status`](Self::status) and the log.
    pub fn start(combo: &str, on_press: impl Fn() + Send + Sync + 'static) -> Self {
        let status = Arc::new(Mutex::new(Status::Starting));
        #[cfg(target_os = "linux")]
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            let (trigger, shared) = (portal_trigger(combo), status.clone());
            let spawned = std::thread::Builder::new().name("hotkey".into()).spawn(move || {
                if let Err(e) = ashpd::zbus::block_on(portal(&trigger, &shared, &on_press)) {
                    log::warn!("riven hotkey unavailable (desktop portal global shortcuts): {e}");
                    *shared.lock().unwrap() = Status::Failed(e.to_string());
                }
            });
            if let Err(e) = spawned {
                *status.lock().unwrap() = Status::Failed(e.to_string());
            }
            return Self { status, native: None, desktop_key: Mutex::new(None) };
        }
        let native = match GlobalHotKeyManager::new() {
            Ok(manager) => {
                let id = Arc::new(AtomicU32::new(0));
                let handler_id = id.clone();
                GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                    if e.id() == handler_id.load(Ordering::Relaxed) && e.state() == HotKeyState::Pressed {
                        on_press();
                    }
                }));
                Some(Mutex::new(Native { manager, current: None, id }))
            }
            Err(e) => {
                *status.lock().unwrap() = Status::Failed(e.to_string());
                None
            }
        };
        let hotkey = Self { status, native, desktop_key: Mutex::new(None) };
        hotkey.set(combo);
        hotkey
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// The key the desktop bound the portal shortcut to when the portal
    /// doesn't say (Hyprland): looked up again every few seconds, so a
    /// config reload shows.
    pub fn desktop_key(&self) -> Option<String> {
        let mut cached = self.desktop_key.lock().unwrap();
        if let Some((at, key)) = cached.as_ref()
            && at.elapsed() < DESKTOP_KEY_FOR
        {
            return key.clone();
        }
        #[cfg(target_os = "linux")]
        let key = hyprland::bound_key(&portal_shortcut()).or_else(|| hyprland::configured_key(&portal_shortcut()));
        #[cfg(not(target_os = "linux"))]
        let key = None;
        *cached = Some((std::time::Instant::now(), key.clone()));
        key
    }

    /// Whether the desktop owns the key (Wayland), so settings can't change it.
    pub fn desktop_owned(&self) -> bool {
        self.native.is_none() && !matches!(self.status(), Status::Failed(_))
    }

    /// Registers `combo` in place of the current key (Windows, X11, macOS).
    /// When the system refuses it, nothing stays registered and
    /// [`status`](Self::status) says why.
    pub fn set(&self, combo: &str) {
        let Some(native) = &self.native else { return };
        let mut native = native.lock().unwrap();
        if let Some(old) = native.current.take() {
            let _ = native.manager.unregister(old);
        }
        let result = parse(combo).and_then(|key| native.manager.register(key).map(|()| key).map_err(|e| e.to_string()));
        let status = match result {
            Ok(key) => {
                native.id.store(key.id(), Ordering::Relaxed);
                native.current = Some(key);
                log::info!("riven hotkey: {combo}");
                Status::Registered(combo.to_string())
            }
            Err(e) => {
                log::warn!("riven hotkey {combo} not registered: {e}");
                Status::Failed(e)
            }
        };
        *self.status.lock().unwrap() = status;
    }
}

/// The shortcut's full name with the portal: `fyi.fluffypunk.fluffyframe:check-riven`.
#[cfg(target_os = "linux")]
pub fn portal_shortcut() -> String {
    format!("{}:{PORTAL_ID}", fluffyframe_core::config::app_meta::PORTAL_APP_ID)
}

/// Hyprland's binds, over its IPC socket.
#[cfg(target_os = "linux")]
mod hyprland {
    use std::io::{Read, Write};
    use std::time::Duration;

    /// The key of the bind triggering `shortcut` ("Ctrl+Alt+R", "F7"), when
    /// running under Hyprland and a bind can be told to: one whose target
    /// Hyprland lists (`hyprland.conf` binds), or whose description names it
    /// (Hyprland's own, "global <shortcut>", when the bind has none).
    pub fn bound_key(shortcut: &str) -> Option<String> {
        let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
        let socket = std::path::Path::new(&runtime).join("hypr").join(sig).join(".socket.sock");
        let mut stream = std::os::unix::net::UnixStream::connect(socket).ok()?;
        stream.set_read_timeout(Some(Duration::from_millis(300))).ok()?;
        stream.set_write_timeout(Some(Duration::from_millis(300))).ok()?;
        stream.write_all(b"j/binds").ok()?;
        let mut json = String::new();
        stream.read_to_string(&mut json).ok()?;
        find_key(&json, shortcut)
    }

    /// The key a bind line in Hyprland's config gives `shortcut`, for binds
    /// [`bound_key`] can't tell apart (Lua binds list only a handle). Reads
    /// `$XDG_CONFIG_HOME/hypr` (`~/.config/hypr`), skipping hidden folders
    /// (backups); the key as written there.
    pub fn configured_key(shortcut: &str) -> Option<String> {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?
            .join("hypr");
        let mut dirs = vec![(config, 0)];
        while let Some((dir, depth)) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                if path.is_dir() && depth < 4 {
                    dirs.push((path, depth + 1));
                } else if path.extension().is_some_and(|e| e == "lua" || e == "conf")
                    && let Ok(text) = std::fs::read_to_string(&path)
                    && let Some(key) = key_in_config(&text, shortcut)
                {
                    return Some(key);
                }
            }
        }
        None
    }

    /// The key of the first uncommented bind line naming `shortcut`:
    /// `hl.bind("F7", hl.dsp.global("…"))` or `bind = CTRL ALT, R, global, …`.
    pub(super) fn key_in_config(text: &str, shortcut: &str) -> Option<String> {
        text.lines().map(str::trim).filter(|l| l.contains(shortcut) && !l.starts_with("--") && !l.starts_with('#')).find_map(|line| {
            if let Some(args) = line.split_once("bind(").map(|(_, a)| a.trim_start()) {
                let quote = args.chars().next().filter(|c| *c == '"' || *c == '\'')?;
                let combo = args[1..].split(quote).next()?;
                return Some(tidy(&combo.split('+').collect::<Vec<_>>()));
            }
            let (_, rest) = line.split_once('=').filter(|(head, _)| head.trim().starts_with("bind"))?;
            let mut fields = rest.split(',');
            let mods = fields.next()?;
            let key = fields.next()?;
            Some(tidy(&mods.split_whitespace().chain([key]).collect::<Vec<_>>()))
        })
    }

    /// Modifiers and key as settings write them: "Ctrl+Alt+R".
    fn tidy(parts: &[&str]) -> String {
        parts
            .iter()
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|p| match p.to_uppercase().as_str() {
                "CTRL" | "CONTROL" => "Ctrl".to_string(),
                "ALT" => "Alt".to_string(),
                "SHIFT" => "Shift".to_string(),
                "SUPER" | "WIN" | "LOGO" | "MOD4" => "Super".to_string(),
                _ if p.chars().count() == 1 => p.to_uppercase(),
                _ => p.to_string(),
            })
            .collect::<Vec<_>>()
            .join("+")
    }

    pub(super) fn find_key(json: &str, shortcut: &str) -> Option<String> {
        let binds: Vec<serde_json::Value> = serde_json::from_str(json).ok()?;
        let text = |b: &serde_json::Value, k: &str| b.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let bind = binds.iter().find(|b| {
            text(b, "dispatcher").contains("global") && (text(b, "arg") == shortcut || text(b, "description").contains(shortcut))
        })?;
        let mask = bind.get("modmask").and_then(|v| v.as_u64()).unwrap_or(0);
        // Hyprland's modifier bits.
        let mods = [(4, "Ctrl"), (8, "Alt"), (1, "Shift"), (64, "Super")];
        let mut parts: Vec<String> = mods.iter().filter(|(bit, _)| mask & bit != 0).map(|(_, m)| m.to_string()).collect();
        let key = text(bind, "key");
        // Single letters as typed on the keycap.
        parts.push(if key.chars().count() == 1 { key.to_uppercase() } else { key });
        Some(parts.join("+"))
    }
}

/// Settings' "Ctrl+Alt+R" in the portal's notation ("CTRL+ALT+R"; Super is LOGO).
#[cfg(target_os = "linux")]
fn portal_trigger(combo: &str) -> String {
    combo
        .split('+')
        .map(|part| match part.trim().to_uppercase().as_str() {
            "SUPER" | "CMD" | "COMMAND" => "LOGO".to_string(),
            "CONTROL" => "CTRL".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Binds the shortcut with the portal, asking for `trigger`, then runs
/// `on_press` on each activation and keeps `status` showing the key the
/// desktop gives it, for as long as the session lasts.
#[cfg(target_os = "linux")]
async fn portal(trigger: &str, status: &Mutex<Status>, on_press: &(impl Fn() + Send + Sync)) -> Result<(), ashpd::Error> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
    use futures_util::StreamExt;

    enum Signal {
        Pressed(String),
        Changed(Vec<Shortcut>),
    }
    let show = |shortcuts: &[Shortcut]| {
        let key = shortcuts.iter().find(|s| s.id() == PORTAL_ID).map(|s| s.trigger_description().to_string()).unwrap_or_default();
        log::info!("riven hotkey: portal shortcut '{PORTAL_ID}' bound to '{key}'");
        *status.lock().unwrap() = Status::Portal(key);
    };

    register_app().await;
    let portal = GlobalShortcuts::new().await?;
    let session = portal.create_session(Default::default()).await?;
    let shortcut = NewShortcut::new(PORTAL_ID, "Check the riven being cycled").preferred_trigger(trigger);
    let pressed = portal.receive_activated().await?.map(|a| Signal::Pressed(a.shortcut_id().to_string()));
    let changed = portal.receive_shortcuts_changed().await?.map(|c| Signal::Changed(c.shortcuts().to_vec()));
    let bound = portal.bind_shortcuts(&session, &[shortcut], None, Default::default()).await?.response()?;
    show(bound.shortcuts());
    let mut signals = futures_util::stream::select(pressed, changed);
    while let Some(signal) = signals.next().await {
        match signal {
            Signal::Pressed(id) if id == PORTAL_ID => on_press(),
            Signal::Pressed(_) => {}
            Signal::Changed(shortcuts) => show(&shortcuts),
        }
    }
    drop(session);
    Ok(())
}

/// Gives the portal a fixed app id ([`PORTAL_APP_ID`]) instead of one guessed
/// from how the app was started (often none), so the shortcut always has the
/// same name. The portal only accepts an id with a desktop entry, which is
/// written when missing. Failing leaves the guessed id.
///
/// [`PORTAL_APP_ID`]: fluffyframe_core::config::app_meta::PORTAL_APP_ID
#[cfg(target_os = "linux")]
async fn register_app() {
    use fluffyframe_core::config::app_meta::PORTAL_APP_ID;
    if let Err(e) = desktop_entry::ensure() {
        log::warn!("writing the desktop entry: {e}");
    }
    let registered = match PORTAL_APP_ID.parse::<ashpd::AppID>() {
        Ok(id) => ashpd::register_host_app(id).await,
        Err(e) => Err(e),
    };
    match registered {
        Ok(()) => log::info!("portal app id: {PORTAL_APP_ID}"),
        Err(e) => log::warn!("portal app id {PORTAL_APP_ID} not registered: {e}"),
    }
}

/// The desktop entry the portal looks the app id up in.
#[cfg(target_os = "linux")]
mod desktop_entry {
    use std::path::PathBuf;

    use fluffyframe_core::config::app_meta::{APP_ID, APP_NAME, PORTAL_APP_ID};

    /// `$XDG_DATA_HOME/applications/<id>.desktop` (or under ~/.local/share).
    fn path() -> Option<PathBuf> {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
        Some(data.join("applications").join(format!("{PORTAL_APP_ID}.desktop")))
    }

    /// Writes the entry unless one exists (the user's own, or a package's in
    /// the system directories): an existing entry is never replaced.
    pub fn ensure() -> std::io::Result<()> {
        let name = format!("{PORTAL_APP_ID}.desktop");
        let system = ["/usr/share/applications", "/usr/local/share/applications"];
        if system.iter().any(|d| std::path::Path::new(d).join(&name).is_file()) {
            return Ok(());
        }
        let Some(path) = path() else { return Ok(()) };
        if path.is_file() {
            return Ok(());
        }
        let exe = std::env::current_exe()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, contents(&exe.to_string_lossy()))?;
        log::info!("wrote {}", path.display());
        Ok(())
    }

    pub(super) fn contents(exe: &str) -> String {
        format!(
            "[Desktop Entry]\nType=Application\nName={APP_NAME}\nComment=Warframe companion\nExec={}\nTerminal=false\nCategories=Game;\nStartupWMClass={APP_ID}\n",
            quote(exe)
        )
    }

    /// An `Exec` argument, quoted with the characters the spec reserves escaped.
    fn quote(arg: &str) -> String {
        let escaped: String = arg
            .chars()
            .flat_map(|c| match c {
                '"' | '`' | '$' | '\\' => vec!['\\', c],
                // `%` starts a field code; `%%` is a literal one.
                '%' => vec!['%', '%'],
                c => vec![c],
            })
            .collect();
        format!("\"{escaped}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_keys_the_system_can_register() {
        let ctrl_alt = egui::Modifiers { ctrl: true, alt: true, ..Default::default() };
        assert_eq!(combo_of(ctrl_alt, egui::Key::R).as_deref(), Some("Ctrl+Alt+R"));
        assert_eq!(combo_of(egui::Modifiers::SHIFT, egui::Key::F5).as_deref(), Some("Shift+F5"));
        assert_eq!(combo_of(egui::Modifiers::NONE, egui::Key::ArrowUp).as_deref(), Some("Up"));
        assert!(parse(&fluffyframe_core::config::settings::Settings::default().riven_hotkey).is_ok());
        assert!(parse("Ctrl+Nope").is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_entry_quotes_the_path() {
        let entry = desktop_entry::contents("/home/me/My Apps/fluffy$frame");
        assert!(entry.contains("Exec=\"/home/me/My Apps/fluffy\\$frame\"\n"), "{entry}");
        assert!(entry.contains("StartupWMClass=fluffyframe\n"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn finds_the_hyprland_bind() {
        let name = "fyi.fluffypunk.fluffyframe:check-riven";
        let bind = |modmask: u32, key: &str, description: &str, arg: &str| {
            format!(r#"{{"modmask": {modmask}, "key": "{key}", "description": "{description}", "dispatcher": "HL.Dispatcher(global)", "arg": "{arg}"}}"#)
        };
        // Lua binds: the target only in Hyprland's own description.
        let json = format!("[{}, {}]", bind(68, "space", "global menu.kando.Kando:main-menu", "331"), bind(12, "r", &format!("global {name}"), "334"));
        assert_eq!(hyprland::find_key(&json, name).as_deref(), Some("Ctrl+Alt+R"));
        // hyprland.conf binds: the target is the argument.
        let json = format!("[{}]", bind(0, "F7", "", name));
        assert_eq!(hyprland::find_key(&json, name).as_deref(), Some("F7"));
        // A description of the user's own hides it.
        let json = format!("[{}]", bind(0, "F7", "FluffyFrame: Trigger Reroll Check", "334"));
        assert_eq!(hyprland::find_key(&json, name), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn finds_the_bind_in_hyprland_config() {
        let name = "fyi.fluffypunk.fluffyframe:check-riven";
        let key = |text: &str| hyprland::key_in_config(text, name);
        assert_eq!(key(r#"hl.bind("F7", hl.dsp.global("fyi.fluffypunk.fluffyframe:check-riven"))"#).as_deref(), Some("F7"));
        assert_eq!(
            key(r#"  hl.bind("SUPER + SHIFT + r", hl.dsp.global("fyi.fluffypunk.fluffyframe:check-riven"), { description = "x" })"#).as_deref(),
            Some("Super+Shift+R")
        );
        assert_eq!(key("bind = CTRL ALT, R, global, fyi.fluffypunk.fluffyframe:check-riven").as_deref(), Some("Ctrl+Alt+R"));
        // Commented out, or another app's: not it.
        assert_eq!(key(r#"-- hl.bind("F7", hl.dsp.global("fyi.fluffypunk.fluffyframe:check-riven"))"#), None);
        assert_eq!(key("# bind = , F8, global, fyi.fluffypunk.fluffyframe:check-riven"), None);
        assert_eq!(key(r#"hl.bind("F9", hl.dsp.global("menu.kando.Kando:main-menu"))"#), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn portal_notation() {
        assert_eq!(portal_trigger("Ctrl+Alt+R"), "CTRL+ALT+R");
        assert_eq!(portal_trigger("Super+Shift+F5"), "LOGO+SHIFT+F5");
    }
}
