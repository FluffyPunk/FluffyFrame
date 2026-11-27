//! warframe.market status (original: `services/wfmPresence.ts`): the one the user
//! picks, and, when that's turned on, "In game" from logging in to Warframe until
//! it closes and "Invisible" when the app quits. The status the game replaced is
//! put back when it closes, unless the user picked one since.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError};
use fluffyframe_core::services::api_helper_runner::game_running;
use fluffyframe_core::services::wfm_client;
use fluffyframe_core::services::wfm_presence::{self, WfmStatus};

use super::{Emitter, Event, Shared, ToastKind};

pub enum PresenceCmd {
    Set(WfmStatus),
}

/// How often the game process and the session are checked.
const POLL: Duration = Duration::from_secs(5);

/// What the worker knows between checks.
#[derive(Default)]
struct Presence {
    status: Option<WfmStatus>,
    /// The session token the status was read for.
    read_for: Option<String>,
    /// Whether the user was logged in to a running game at the last check.
    was_playing: Option<bool>,
    /// Set while "In game" is the game's doing: the status to put back (None:
    /// unknown, so invisible).
    game_override: Option<Option<WfmStatus>>,
}

pub fn run(shared: Arc<Shared>, emitter: Emitter, rx: Receiver<PresenceCmd>) {
    let mut p = Presence::default();
    loop {
        let session = wfm_client::session();
        match &session {
            None if p.read_for.is_some() => {
                p = Presence { was_playing: p.was_playing, ..Default::default() };
                emitter.emit(Event::WfmStatus(None));
            }
            Some(s) if p.read_for.as_deref() != Some(s.token.as_str()) => {
                p.read_for = Some(s.token.clone());
                if let Some(profile) = s.slug.clone().or(s.ingame_name.clone()) {
                    match wfm_presence::current(&profile) {
                        Ok(status) => p.status = status,
                        Err(e) => log::warn!("warframe.market status: {e:#}"),
                    }
                }
                emitter.emit(Event::WfmStatus(p.status));
            }
            _ => {}
        }

        // Logged in (EE.log) to a game that is still running: the login flag
        // isn't cleared when the game closes, only when it next starts.
        let playing = game_running() && shared.logged_in.load(Ordering::Relaxed);
        if let Some(s) = &session
            && shared.settings.read().unwrap().wfm_auto_status
        {
            if playing && p.was_playing != Some(true) {
                log::info!("Logged in to Warframe: warframe.market status in game");
                p.game_override = Some(p.status.filter(|s| *s != WfmStatus::InGame));
                if p.status != Some(WfmStatus::InGame) {
                    push(&mut p, &emitter, &s.token, WfmStatus::InGame);
                }
            } else if !playing && p.was_playing == Some(true)
                && let Some(restore) = p.game_override.take()
            {
                let restore = restore.unwrap_or(WfmStatus::Invisible);
                log::info!("Warframe closed: warframe.market status back to {}", restore.label());
                push(&mut p, &emitter, &s.token, restore);
            }
        }
        p.was_playing = Some(playing);

        match rx.recv_timeout(POLL) {
            Ok(PresenceCmd::Set(status)) => match wfm_client::session() {
                Some(s) => {
                    // The user's pick wins: closing the game won't replace it.
                    p.game_override = None;
                    push(&mut p, &emitter, &s.token, status);
                }
                None => emitter.toast(ToastKind::Error, "Sign in to warframe.market first"),
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// On app quit: invisible, when the automatic status is on. Waits at most a few
/// seconds so a slow network can't hold up the exit.
pub fn on_app_exit(shared: &Shared) {
    if !shared.settings.read().unwrap().wfm_auto_status {
        return;
    }
    let Some(session) = wfm_client::session() else { return };
    let (tx, rx) = crossbeam_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send(wfm_presence::set(&session.token, WfmStatus::Invisible));
    });
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => log::info!("app quit: warframe.market status invisible"),
        Ok(Err(e)) => log::warn!("app quit: warframe.market status: {e:#}"),
        Err(_) => log::warn!("app quit: warframe.market status timed out"),
    }
}

fn push(p: &mut Presence, emitter: &Emitter, token: &str, status: WfmStatus) {
    match wfm_presence::set(token, status) {
        Ok(()) => {
            p.status = Some(status);
            emitter.emit(Event::WfmStatus(p.status));
        }
        Err(e) => emitter.toast(ToastKind::Error, format!("warframe.market status: {e}")),
    }
}
