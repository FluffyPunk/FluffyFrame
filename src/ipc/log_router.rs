//! Routes EE.log events to side effects (original: `ipc/tradeWorkflow.ts`,
//! `ipc/messageNotificationIpc.ts`, the login hook in `apiHelperRunner.ts`).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use chrono::Utc;
use crossbeam_channel::{Receiver, Sender, select, unbounded};
use fluffyframe_core::services::ee_log_monitor::{self, LogEvent, TradeKind};
use fluffyframe_core::services::notifications;
use fluffyframe_core::services::trade_tracker::{TradeEvent, TradeLog};

use super::{Emitter, Event, InventoryCmd, Shared};
use crate::i18n::{tr, trf};

pub enum LogCmd {
    /// The EE.log path setting changed.
    Restart,
}

pub fn run(shared: Arc<Shared>, emitter: Emitter, rx: Receiver<LogCmd>, inventory: Sender<InventoryCmd>) {
    let mut trades = TradeLog::load();
    loop {
        let (tx, events) = unbounded();
        let path = shared.settings.read().unwrap().ee_log_path.clone();
        let monitor = ee_log_monitor::spawn(path, tx);
        let restart = loop {
            select! {
                recv(events) -> ev => {
                    let Ok(ev) = ev else { break false };
                    handle(&shared, &emitter, &inventory, &mut trades, &ev);
                    emitter.emit(Event::Log(ev));
                }
                recv(rx) -> cmd => match cmd {
                    Ok(LogCmd::Restart) => break true,
                    Err(_) => break false,
                },
            }
        };
        monitor.stop();
        if !restart {
            return;
        }
    }
}

fn handle(shared: &Shared, emitter: &Emitter, inventory: &Sender<InventoryCmd>, trades: &mut TradeLog, ev: &LogEvent) {
    match ev {
        LogEvent::LoginComplete => {
            shared.logged_in.store(true, Ordering::Relaxed);
            let _ = inventory.send(InventoryCmd::LoginDetected);
        }
        LogEvent::GameStarted => shared.logged_in.store(false, Ordering::Relaxed),
        LogEvent::PlayerName(name) => {
            let mut s = shared.settings.write().unwrap();
            if s.player_name.as_ref() != Some(name) {
                s.player_name = Some(name.clone());
                if let Err(e) = s.save() {
                    log::warn!("saving settings: {e}");
                }
            }
        }
        LogEvent::Whisper(from) => {
            if shared.settings.read().unwrap().notify_whispers {
                notifications::send(tr("New whisper"), &trf("{} sent you a message", &[&from]));
            }
        }
        LogEvent::TradeCompleted(trade) => {
            let at = Utc::now();
            if trades.record(trade.clone(), at) {
                trades.save();
                if shared.settings.read().unwrap().notify_trades {
                    let what = match trade.kind {
                        TradeKind::Sale => trf("Sold to {} for {}p", &[&trade.partner, &trade.platinum]),
                        TradeKind::Purchase => trf("Bought from {} for {}p", &[&trade.partner, &trade.platinum]),
                        TradeKind::Trade => trf("Traded with {}", &[&trade.partner]),
                    };
                    notifications::send(tr("Trade complete"), &what);
                }
                emitter.emit(Event::Trade(TradeEvent { at, trade: trade.clone() }));
                let _ = inventory.send(InventoryCmd::GameChanged("trade"));
            }
        }
        // Things that change the inventory: re-read it (debounced by the worker).
        LogEvent::MissionEnded => {
            let _ = inventory.send(InventoryCmd::GameChanged("mission ended"));
        }
        LogEvent::InventorySynced => {
            let _ = inventory.send(InventoryCmd::GameChanged("game synced inventory"));
        }
        LogEvent::PurchaseCompleted => {
            let _ = inventory.send(InventoryCmd::GameChanged("purchase"));
        }
        LogEvent::RivenCycled => {
            let _ = inventory.send(InventoryCmd::RivenCycleSent { pending: true });
        }
        LogEvent::RivenChosen => {
            let _ = inventory.send(InventoryCmd::RivenCycleSent { pending: false });
        }
        LogEvent::Attached(_) => {}
    }
}
