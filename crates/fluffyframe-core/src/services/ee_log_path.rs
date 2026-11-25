//! Locate EE.log on Windows, Steam Proton, or another launcher's Wine prefix
//! (original: `services/eeLogPath.ts`). `FLUFFYFRAME_EE_LOG` overrides discovery.

use std::path::{Path, PathBuf};

const WARFRAME_STEAM_APP_ID: &str = "230410";

/// Every `"path"` value in Steam's libraryfolders.vdf.
pub fn parse_steam_library_paths(vdf: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""path"\s+"([^"]*)""#).unwrap();
    re.captures_iter(vdf).map(|c| c[1].replace("\\\\", "\\")).collect()
}

/// The `prefix:` line of a Lutris game config.
pub fn parse_lutris_prefix(yml: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^[ \t]*prefix:[ \t]*(\S.*?)[ \t]*$"#).unwrap();
    let raw = re.captures(yml)?.get(1)?.as_str();
    let unquoted = raw.trim_matches(|c| c == '"' || c == '\'');
    (!unquoted.is_empty()).then(|| unquoted.to_string())
}

fn wine_prefix_log(prefix: &Path) -> Vec<PathBuf> {
    let users = prefix.join("drive_c").join("users");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&users) {
        for e in entries.flatten() {
            out.push(e.path().join("AppData").join("Local").join("Warframe").join("EE.log"));
        }
    }
    out
}

fn home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf())
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|r| r.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// All candidate EE.log locations, most likely first.
pub fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = crate::config::app_meta::env_var("EE_LOG") {
        out.push(PathBuf::from(p));
    }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            out.push(PathBuf::from(local).join("Warframe").join("EE.log"));
        }
        return out;
    }
    let Some(home) = home() else { return out };

    // Steam (native, Flatpak, Snap) and every library it knows about.
    let steam_roots = [
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join("snap/steam/common/.local/share/Steam"),
    ];
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in &steam_roots {
        libraries.push(root.clone());
        if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            libraries.extend(parse_steam_library_paths(&vdf).into_iter().map(PathBuf::from));
        }
    }
    libraries.dedup();
    for lib in libraries {
        out.push(
            lib.join("steamapps/compatdata")
                .join(WARFRAME_STEAM_APP_ID)
                .join("pfx/drive_c/users/steamuser/AppData/Local/Warframe/EE.log"),
        );
    }

    // Heroic, Lutris, Bottles, plain Wine.
    let mut prefixes = vec![home.join(".wine")];
    for dir in [
        home.join(".config/heroic/GamesConfig"),
        home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/GamesConfig"),
    ] {
        for file in subdirs(&dir).into_iter().filter(|p| p.extension().is_some_and(|e| e == "json")) {
            if let Ok(v) = std::fs::read_to_string(&file)
                .map_err(anyhow::Error::from)
                .and_then(|t| Ok(serde_json::from_str::<serde_json::Value>(&t)?))
                && let Some(obj) = v.as_object()
            {
                for entry in obj.values() {
                    if let Some(p) = entry.get("winePrefix").and_then(|p| p.as_str()) {
                        prefixes.push(PathBuf::from(p));
                    }
                }
            }
        }
    }
    prefixes.extend(subdirs(&home.join("Games/Heroic/Prefixes/default")));
    for dir in [
        home.join(".config/lutris/games"),
        home.join(".local/share/lutris/games"),
        home.join(".var/app/net.lutris.Lutris/config/lutris/games"),
        home.join(".var/app/net.lutris.Lutris/data/lutris/games"),
    ] {
        for file in subdirs(&dir) {
            if file.extension().is_some_and(|e| e == "yml" || e == "yaml")
                && let Some(p) = std::fs::read_to_string(&file).ok().and_then(|t| parse_lutris_prefix(&t))
            {
                prefixes.push(PathBuf::from(p));
            }
        }
    }
    for dir in [
        home.join(".local/share/bottles/bottles"),
        home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
    ] {
        prefixes.extend(subdirs(&dir));
    }
    for prefix in prefixes {
        out.extend(wine_prefix_log(&prefix));
    }
    out
}

/// The existing EE.log modified most recently.
pub fn resolve(override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = override_path.filter(|p| !p.trim().is_empty()) {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    candidates()
        .into_iter()
        .filter_map(|p| {
            let modified = std::fs::metadata(&p).ok()?.modified().ok()?;
            Some((modified, p))
        })
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
}

/// EE.log logs "Logging in as <name>" once the session is authenticated.
pub fn contains_login_marker(path: &Path) -> bool {
    use std::io::Read;
    const MARKER: &[u8] = b"Logging in as ";
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let mut buf = vec![0u8; 4 * 1024 * 1024];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) | Err(_) => return false,
            Ok(n) => n,
        };
        carry.extend_from_slice(&buf[..n]);
        if memfind(&carry, MARKER) {
            return true;
        }
        let keep = carry.len().saturating_sub(MARKER.len());
        carry.drain(..keep);
    }
}

fn memfind(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_vdf() {
        let vdf = r#""libraryfolders" { "0" { "path" "/home/u/.local/share/Steam" } "1" { "path" "/mnt/games" } }"#;
        assert_eq!(parse_steam_library_paths(vdf), vec!["/home/u/.local/share/Steam", "/mnt/games"]);
    }

    #[test]
    fn lutris_prefix() {
        assert_eq!(parse_lutris_prefix("game:\n  prefix: '/home/u/Games/wf'\n"), Some("/home/u/Games/wf".into()));
        assert_eq!(parse_lutris_prefix("game:\n  exe: x\n"), None);
    }

    #[test]
    fn login_marker() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("EE.log");
        std::fs::write(&p, "123.4 Sys [Info]: Logging in as Tenno\n").unwrap();
        assert!(contains_login_marker(&p));
        std::fs::write(&p, "nothing").unwrap();
        assert!(!contains_login_marker(&p));
    }
}
