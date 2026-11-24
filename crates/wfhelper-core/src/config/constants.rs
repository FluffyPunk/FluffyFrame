//! Endpoints and timings (original: `config/game/constants.ts`, `config/runtime/*`).

use std::time::Duration;

// DE public export: an LZMA index of hash-suffixed manifest names.
pub const PUBLIC_EXPORT_INDEX_URL: &str =
    "https://content.warframe.com/PublicExport/index_en.txt.lzma";
pub const PUBLIC_EXPORT_MANIFEST_BASE: &str = "https://content.warframe.com/PublicExport/Manifest/";
/// Prefix for `textureLocation` values from ExportManifest.json.
pub const PUBLIC_EXPORT_IMAGE_BASE: &str = "https://content.warframe.com/PublicExport";
/// WFHelper's icon mirror; hosts the wiki mod/arcane card art.
pub const ICON_MIRROR_BASE: &str = "https://assets.wfhelper.com";

/// PC world state. The old `content.warframe.com/dynamic/worldState.php` now 404s.
pub const WORLD_STATE_URL: &str = "https://api.warframe.com/cdn/worldState.php";
pub const ARBITRATION_SCHEDULE_URL: &str = "https://browse.wf/arbys.txt";
pub const DROPS_URL: &str = "https://drops.warframestat.us/data/all.json";

pub const WFM_V1: &str = "https://api.warframe.market/v1";
pub const WFM_V2: &str = "https://api.warframe.market/v2";
pub const WFM_SITE: &str = "https://warframe.market";
pub const WFM_ASSETS: &str = "https://warframe.market/static/assets/";
pub const WIKI_BASE: &str = "https://wiki.warframe.com/w/";

/// warframe.market allows about 3 requests per second.
pub const WFM_MIN_REQUEST_GAP: Duration = Duration::from_millis(350);
pub const WFM_RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(30);
pub const WFM_PRICE_TTL: Duration = Duration::from_secs(6 * 3600);
pub const WFM_CATALOG_TTL: Duration = Duration::from_secs(24 * 3600);

pub const WORLD_STATE_REFRESH: Duration = Duration::from_secs(60);
pub const ARBI_SCHEDULE_TTL: Duration = Duration::from_secs(24 * 3600);
pub const DROPS_TTL: Duration = Duration::from_secs(3 * 24 * 3600);

/// Automatic inventory refreshes wait this long between helper runs.
pub const DEFAULT_HELPER_POLL: Duration = Duration::from_secs(10 * 60);
/// Failures that never reached DE's API retry sooner.
pub const HELPER_LOCAL_FAILURE_RETRY: Duration = Duration::from_secs(90);

pub const EE_LOG_POLL: Duration = Duration::from_millis(500);
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(60);
