use std::collections::{HashMap, HashSet, VecDeque};

use broker_api::{ClientOrderId, Execution, Intent, PlaceIntent, Tick};
use tradecore::PriceTicks;

#[derive(Debug, Default)]
pub struct TickGate {
    last_ts: HashMap<String, i64>,
    last_px: HashMap<String, i64>,
    pub band: i64,
}

impl TickGate {
    pub fn new(band: i64) -> Self {
        Self {
            band,
            ..Self::default()
        }
    }

    pub fn accept(&mut self, tick: &Tick) -> bool {
        let key = tick.instrument.0.clone();
        if let Some(previous) = self.last_ts.get(&key) {
            if tick.exchange_ts_ms < *previous {
                return false;
            }
        }
        if let Some(previous) = self.last_px.get(&key) {
            if previous.abs_diff(tick.price.get()) > self.band as u64 {
                return false;
            }
        }
        self.last_ts.insert(key.clone(), tick.exchange_ts_ms);
        self.last_px.insert(key, tick.price.get());
        true
    }
}

#[derive(Debug)]
pub struct Arbiter {
    last_rx: HashMap<String, i64>,
    last_forward: HashMap<String, i64>,
    disabled: HashSet<String>,
    stale_after_ms: i64,
}

impl Arbiter {
    pub fn new(stale_after_ms: i64) -> Self {
        Self {
            last_rx: HashMap::new(),
            last_forward: HashMap::new(),
            disabled: HashSet::new(),
            stale_after_ms,
        }
    }

    pub fn set_enabled(&mut self, feed: &str, enabled: bool) {
        if enabled {
            self.disabled.remove(feed);
        } else {
            self.disabled.insert(feed.to_string());
        }
    }

    pub fn on_tick(&mut self, tick: &Tick, now_ms: i64) -> Option<Tick> {
        self.last_rx.insert(tick.feed.clone(), now_ms);
        if self.disabled.contains(&tick.feed) {
            return None;
        }
        let key = tick.instrument.0.clone();
        if let Some(previous) = self.last_forward.get(&key) {
            if tick.exchange_ts_ms < *previous {
                return None;
            }
        }
        self.last_forward.insert(key, tick.exchange_ts_ms);
        Some(tick.clone())
    }

    pub fn stale_feeds(&self, now_ms: i64) -> Vec<String> {
        self.last_rx
            .iter()
            .filter(|(_, seen)| now_ms.saturating_sub(**seen) >= self.stale_after_ms)
            .map(|(feed, _)| feed.clone())
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct Journal {
    pub ids: Vec<String>,
}

#[derive(Debug)]
struct Inflight {
    sent: i64,
    pending: Option<i64>,
}

#[derive(Debug)]
pub struct OrderManager<E> {
    pub exec: E,
    pub journal: Journal,
    inflight: HashMap<String, Inflight>,
    placed: HashSet<String>,
}

impl<E: Execution> OrderManager<E> {
    pub fn new(exec: E) -> Self {
        Self {
            exec,
            journal: Journal::default(),
            inflight: HashMap::new(),
            placed: HashSet::new(),
        }
    }

    pub fn submit(&mut self, intent: Intent) {
        match intent {
            Intent::Place(place) => self.place(place),
            Intent::Modify {
                client_id, trigger, ..
            } => self.modify(client_id, trigger),
            Intent::Cancel { client_id } => {
                self.journal.ids.push(format!("cancel:{}", client_id.0));
                self.exec.cancel(&client_id);
            }
        }
    }

    pub fn on_replaced(&mut self, client_id: &ClientOrderId, trigger: PriceTicks) {
        let Some(state) = self.inflight.get_mut(&client_id.0) else {
            return;
        };
        let pending = state.pending.take();
        if let Some(next) = pending {
            if next != trigger.get() {
                state.sent = next;
                let next_px = PriceTicks::new(next);
                self.journal
                    .ids
                    .push(format!("modify:{}:{next}", client_id.0));
                self.exec.modify(client_id, next_px);
                return;
            }
        }
        self.inflight.remove(&client_id.0);
    }

    pub fn on_timeout(&mut self, client_id: &ClientOrderId) -> bool {
        self.exec.knows(client_id) || self.placed.contains(&client_id.0)
    }

    fn place(&mut self, place: PlaceIntent) {
        if self.placed.contains(&place.client_id.0) || self.exec.knows(&place.client_id) {
            return;
        }
        self.journal.ids.push(place.client_id.0.clone());
        self.placed.insert(place.client_id.0.clone());
        self.exec.place(&place);
    }

    fn modify(&mut self, client_id: ClientOrderId, trigger: PriceTicks) {
        if let Some(state) = self.inflight.get_mut(&client_id.0) {
            state.pending = Some(trigger.get());
            return;
        }
        self.journal
            .ids
            .push(format!("modify:{}:{}", client_id.0, trigger.get()));
        self.inflight.insert(
            client_id.0.clone(),
            Inflight {
                sent: trigger.get(),
                pending: None,
            },
        );
        self.exec.modify(&client_id, trigger);
    }
}

#[derive(Debug)]
pub struct RateBucket {
    times: VecDeque<i64>,
    limit: usize,
}

impl RateBucket {
    pub fn per_second(limit: usize) -> Self {
        Self {
            times: VecDeque::new(),
            limit,
        }
    }

    /// `true` when the call is sent now. `false` means the caller must wait and keep the intent.
    pub fn try_send(&mut self, now_ms: i64) -> bool {
        while self
            .times
            .front()
            .is_some_and(|stamp| now_ms.saturating_sub(*stamp) >= 1_000)
        {
            self.times.pop_front();
        }
        if self.times.len() >= self.limit {
            return false;
        }
        self.times.push_back(now_ms);
        true
    }
}
