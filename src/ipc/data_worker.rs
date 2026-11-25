//! Game data, catalog, world state and schedule notifications
//! (original: `ipc/worldStateIpc.ts`, `ipc/arbiScheduleIpc.ts`, `services/relicDataUpdate.ts`).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use fluffyframe_core::config::constants::WORLD_STATE_REFRESH;
use fluffyframe_core::services::arbi_schedule::ArbiSchedule;
use fluffyframe_core::services::item_database::ItemDb;
use fluffyframe_core::services::world_state::{self, WorldState};
use fluffyframe_core::services::{drop_data, notifications, public_export, riven_good_rolls, wfm_catalog};

use super::{Emitter, Event, Shared, ToastKind};

pub enum DataCmd {
    RefreshWorld,
    LoadDrops,
    ReloadGameData,
}

/// How long before a favorite arbitration starts to notify.
const ARBI_LEAD: chrono::Duration = chrono::Duration::minutes(5);

fn load_game_data(shared: &Shared, emitter: &Emitter) -> Option<Arc<ItemDb>> {
    emitter.emit(Event::Loading(Some("Loading game data from DE…".into())));
    let result = public_export::load(&|name| {
        emitter.emit(Event::Loading(Some(format!("Loading game data: {name}"))));
    });
    match result {
        Ok(exports) => {
            let db = Arc::new(ItemDb::build(&exports));
            *shared.db.write().unwrap() = Some(db.clone());
            emitter.emit(Event::GameData(db.clone()));
            Some(db)
        }
        Err(e) => {
            log::error!("game data: {e:#}");
            emitter.toast(ToastKind::Error, format!("Game data failed to load: {e}"));
            None
        }
    }
}

pub fn run(shared: Arc<Shared>, emitter: Emitter, rx: Receiver<DataCmd>) {
    let mut db = load_game_data(&shared, &emitter);

    emitter.emit(Event::Loading(Some("Loading warframe.market catalog…".into())));
    match wfm_catalog::load() {
        Ok(c) => emitter.emit(Event::Catalog(Arc::new(c))),
        Err(e) => emitter.toast(ToastKind::Error, format!("warframe.market catalog unavailable: {e}")),
    }
    emitter.emit(Event::Loading(None));

    let arbi = match ArbiSchedule::load() {
        Ok(a) => {
            let a = Arc::new(a);
            emitter.emit(Event::Arbi(a.clone()));
            Some(a)
        }
        Err(e) => {
            log::warn!("arbitration schedule: {e:#}");
            None
        }
    };

    // Drop tables decide which relics are vaulted, so load them up front.
    match drop_data::DropIndex::load() {
        Ok(d) => emitter.emit(Event::Drops(Arc::new(d))),
        Err(e) => log::warn!("drop tables: {e:#}"),
    }

    match riven_good_rolls::GoodRolls::load() {
        Ok(g) => emitter.emit(Event::GoodRolls(Arc::new(g))),
        Err(e) => log::warn!("riven good rolls: {e:#}"),
    }
    // The game export and drop table JSON are parsed and dropped by now.
    crate::memory::trim();

    let mut notified_arbi: HashSet<i64> = HashSet::new();
    let mut seen_fissures: Option<HashSet<String>> = None;
    let mut next_world = Instant::now();
    loop {
        if Instant::now() >= next_world {
            if let Some(db) = &db {
                match world_state::fetch_raw() {
                    Ok(raw) => {
                        let ws = world_state::parse(&raw, db);
                        notify_fissures(&shared, &ws, &mut seen_fissures);
                        emitter.emit(Event::World(Arc::new(ws)));
                    }
                    Err(e) => log::warn!("world state: {e:#}"),
                }
            }
            if let (Some(arbi), Some(db)) = (&arbi, &db) {
                notify_arbitration(&shared, arbi, db, &mut notified_arbi);
            }
            next_world = Instant::now() + WORLD_STATE_REFRESH;
        }
        let wait = next_world.saturating_duration_since(Instant::now()).max(Duration::from_millis(50));
        match rx.recv_timeout(wait) {
            Ok(DataCmd::RefreshWorld) => next_world = Instant::now(),
            Ok(DataCmd::ReloadGameData) => {
                db = load_game_data(&shared, &emitter).or(db);
                emitter.emit(Event::Loading(None));
                next_world = Instant::now();
            }
            Ok(DataCmd::LoadDrops) => {
                emitter.emit(Event::Loading(Some("Loading drop tables…".into())));
                match drop_data::DropIndex::load() {
                    Ok(d) => emitter.emit(Event::Drops(Arc::new(d))),
                    Err(e) => emitter.toast(ToastKind::Error, format!("Drop tables unavailable: {e}")),
                }
                emitter.emit(Event::Loading(None));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn notify_arbitration(shared: &Shared, arbi: &ArbiSchedule, db: &ItemDb, notified: &mut HashSet<i64>) {
    let settings = shared.settings.read().unwrap();
    if !settings.notify_arbitrations || settings.arbi_favorites.is_empty() {
        return;
    }
    let now = Utc::now();
    if let Some(slot) = arbi.next_on(now - chrono::Duration::minutes(1), &settings.arbi_favorites)
        && slot.start - now <= ARBI_LEAD
        && notified.insert(slot.start.timestamp())
    {
        let mission = db.regions.get(&slot.node_id).map(|r| r.mission_type.clone()).unwrap_or_default();
        notifications::send(
            "Favorite arbitration",
            &format!("{} {} starts at {}", db.node_label(&slot.node_id), mission, slot.start.with_timezone(&chrono::Local).format("%H:%M")),
        );
    }
}

fn notify_fissures(shared: &Shared, ws: &WorldState, seen: &mut Option<HashSet<String>>) {
    let ids: HashSet<String> = ws
        .fissures
        .iter()
        .map(|f| format!("{}|{}|{}|{:?}", f.node_id, f.tier, f.steel_path, f.expiry))
        .collect();
    let settings = shared.settings.read().unwrap();
    // The first fetch only primes the set: no burst of alerts on startup.
    if let Some(previous) = seen.as_ref()
        && settings.notify_fissures
        && !settings.fissure_alert_missions.is_empty()
    {
        for f in &ws.fissures {
            let id = format!("{}|{}|{}|{:?}", f.node_id, f.tier, f.steel_path, f.expiry);
            let wanted = settings.fissure_alert_missions.iter().any(|m| m.eq_ignore_ascii_case(&f.mission_type))
                && (!settings.fissure_alert_steel_path || f.steel_path);
            if wanted && !previous.contains(&id) {
                notifications::send(
                    "Void fissure",
                    &format!("{} {}{} on {}", f.tier, f.mission_type, if f.steel_path { " (Steel Path)" } else { "" }, f.node),
                );
            }
        }
    }
    *seen = Some(ids);
}
