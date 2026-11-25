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
        LogEvent::Whisper(from) => {
            if shared.settings.read().unwrap().notify_whispers {
                notifications::send("New whisper", &format!("{from} sent you a message"));
            }
        }
        LogEvent::TradeCompleted(trade) => {
            let at = Utc::now();
            if trades.record(trade.clone(), at) {
                trades.save();
                if shared.settings.read().unwrap().notify_trades {
                    let what = match trade.kind {
                        TradeKind::Sale => format!("Sold to {} for {}p", trade.partner, trade.platinum),
                        TradeKind::Purchase => format!("Bought from {} for {}p", trade.partner, trade.platinum),
                        TradeKind::Trade => format!("Traded with {}", trade.partner),
                    };
                    notifications::send("Trade complete", &what);
                }
                emitter.emit(Event::Trade(TradeEvent { at, trade: trade.clone() }));
                // Pick up the changed inventory on the next read.
                let _ = inventory.send(InventoryCmd::LoginDetected);
            }
        }
        LogEvent::Attached(_) | LogEvent::MissionEnded => {}
    }
}
