//! Price queue (original: `services/wfmScheduler.ts`, `src/lib/priceLoader.ts`).
//!
//! Requests go to the front of the queue so whatever the user is looking at loads
//! first; cached prices younger than the TTL are never refetched.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use crossbeam_channel::{Receiver, TryRecvError};
use wfhelper_core::services::wfm_stats_price::{self, price_key};

use super::{Emitter, Event, Shared};

pub enum PriceCmd {
    /// (slug, rank) pairs to price.
    Request(Vec<(String, Option<u32>)>),
    /// Drop cached values and refetch these.
    Refresh(Vec<(String, Option<u32>)>),
}

const BATCH: usize = 8;
const SAVE_EVERY: usize = 25;

pub fn run(shared: Arc<Shared>, emitter: Emitter, rx: Receiver<PriceCmd>) {
    // Publish the disk cache right away.
    let cached: Vec<_> = shared.prices.snapshot().into_iter().collect();
    if !cached.is_empty() {
        emitter.emit(Event::Prices(cached));
    }
    let mut queue: VecDeque<(String, Option<u32>)> = VecDeque::new();
    let mut queued: HashSet<String> = HashSet::new();
    let mut failed: HashSet<String> = HashSet::new();
    let mut batch = Vec::new();
    let mut since_save = 0;

    let enqueue = |items: Vec<(String, Option<u32>)>, force: bool, queue: &mut VecDeque<_>, queued: &mut HashSet<String>, failed: &mut HashSet<String>| {
        for (slug, rank) in items.into_iter().rev() {
            let key = price_key(&slug, rank);
            if force {
                failed.remove(&key);
            } else if failed.contains(&key) || shared.prices.get(&key).is_some_and(|p| p.is_fresh()) {
                continue;
            }
            if queued.insert(key) {
                queue.push_front((slug, rank));
            }
        }
    };

    loop {
        // Block when idle, otherwise drain pending commands between fetches.
        let cmd = if queue.is_empty() {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            }
        } else {
            match rx.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        };
        if let Some(cmd) = cmd {
            match cmd {
                PriceCmd::Request(items) => enqueue(items, false, &mut queue, &mut queued, &mut failed),
                PriceCmd::Refresh(items) => enqueue(items, true, &mut queue, &mut queued, &mut failed),
            }
            emitter.emit(Event::PriceQueue(queue.len()));
            continue;
        }

        let Some((slug, rank)) = queue.pop_front() else { continue };
        let key = price_key(&slug, rank);
        queued.remove(&key);
        match wfm_stats_price::fetch(&slug, rank) {
            Ok(Some(price)) => {
                shared.prices.insert(key.clone(), price);
                batch.push((key, price));
                since_save += 1;
            }
            Ok(None) => {
                failed.insert(key);
            }
            Err(e) => {
                log::debug!("price {key}: {e}");
                failed.insert(key);
            }
        }
        if batch.len() >= BATCH || queue.is_empty() {
            emitter.emit(Event::Prices(std::mem::take(&mut batch)));
            emitter.emit(Event::PriceQueue(queue.len()));
        }
        if since_save >= SAVE_EVERY || (queue.is_empty() && since_save > 0) {
            shared.prices.save();
            since_save = 0;
        }
    }
}
