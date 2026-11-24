//! Is the game running? (original: `services/warframeStatus.ts`). Cached briefly,
//! since enumerating processes is not free.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE_FOR: Duration = Duration::from_secs(5);

static CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

pub fn is_running() -> bool {
    let mut guard = CACHE.lock().unwrap();
    if let Some((at, running)) = *guard
        && at.elapsed() < CACHE_FOR
    {
        return running;
    }
    let running = wf_api::game_pid().is_some();
    *guard = Some((Instant::now(), running));
    running
}
