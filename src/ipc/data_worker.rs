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
use fluffyframe_core::services::bounty_cycle::{self, BountyBook};
use fluffyframe_core::services::{drop_data, market_blueprints, notifications, region_mastery, public_export, riven_good_rolls, wfm_catalog};

use super::{Emitter, Event, Shared, ToastKind};

pub enum DataCmd {
    RefreshWorld,
    LoadDrops,
    /// Seeded bounties and bounty text (see `bounty_cycle`).
    LoadBounties,
    ReloadGameData,
}

/// How often planet-cycle alerts are checked.
const CYCLE_CHECK: Duration = Duration::from_secs(10);

/// How long before a favorite arbitration starts to notify.
const ARBI_LEAD: chrono::Duration = chrono::Duration::minutes(5);

fn load_game_data(shared: &Shared, emitter: &Emitter) -> Option<Arc<ItemDb>> {
    emitter.emit(Event::Loading(Some("Loading game data from DE…".into())));
    let result = public_export::load(&|name| {
        emitter.emit(Event::Loading(Some(format!("Loading game data: {name}"))));
    });
    match result {
        Ok(exports) => {
            let mut db = ItemDb::build(&exports);
            db.market_blueprints = market_blueprints::load();
            db.node_mastery = region_mastery::load();
            let db = Arc::new(db);
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
    let mut cycle_watch = notifications::CycleWatch::default();
    // Cycles are clock math: checked often, so alerts and bell toggles take
    // effect within seconds, not at the next world fetch. Cetus (and the Cambion
    // Drift) also need the Ostron bounty expiry from the last fetch.
    let mut cetus_expiry = None;
    let mut next_cycles = Instant::now();
    let mut next_world = Instant::now();
    loop {
        if Instant::now() >= next_world {
            if let Some(db) = &db {
                match world_state::fetch_raw() {
                    Ok(raw) => {
                        let ws = world_state::parse(&raw, db);
                        notify_fissures(&shared, &ws, &mut seen_fissures);
                        cetus_expiry = ws.bounties.iter().find(|b| b.tag == "CetusSyndicate").and_then(|b| b.expiry).or(cetus_expiry);
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
        if Instant::now() >= next_cycles {
            notify_cycles(&shared, &mut cycle_watch, &world_state::cycles(Utc::now(), cetus_expiry));
            next_cycles = Instant::now() + CYCLE_CHECK;
        }
        let wait = next_world.min(next_cycles).saturating_duration_since(Instant::now()).max(Duration::from_millis(50));
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
            Ok(DataCmd::LoadBounties) => match bounty_cycle::load() {
                Ok(b) => emitter.emit(Event::Bounties(Arc::new(b))),
                Err(e) => {
                    log::warn!("bounty cycle: {e:#}");
                    emitter.emit(Event::Bounties(Arc::new(BountyBook::default())));
                }
            },
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

fn notify_cycles(shared: &Shared, watch: &mut notifications::CycleWatch, cycles: &[world_state::Cycle]) {
    let (enabled, lead) = {
        let s = shared.settings.read().unwrap();
        (s.cycle_alerts.clone(), chrono::Duration::minutes(s.cycle_alert_minutes.into()))
    };
    for (title, body) in watch.check(cycles, &enabled, lead, Utc::now()) {
        notifications::send(&title, &body);
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
