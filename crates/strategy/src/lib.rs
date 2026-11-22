use std::collections::HashMap;

use broker_api::{
    ClientOrderId, InstrumentId, Intent, IntentPriority, LegId, OrderEvent, OrderRole, PlaceIntent,
    Product, Side, StrategyId,
};
use tradecore::{PriceTicks, QtyUnits};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Monitor {
    Candle { ms: i64 },
    Ltp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlMode {
    #[default]
    Off,
    Points,
    Percent,
    UnderlyingPoints,
    UnderlyingPercent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailMode {
    #[default]
    Off,
    Points,
    Percent,
}

/// Premium move that must happen before this leg's entry order is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Momentum {
    #[default]
    Off,
    PremiumUp(i64),
    PremiumDown(i64),
    PointsUp(i64),
    PointsDown(i64),
}

/// Per-leg stop and trail. Values are premium ticks, or a whole percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LegRisk {
    pub sl: SlMode,
    pub sl_value: i64,
    pub trail: TrailMode,
    pub trail_every: i64,
    pub trail_by: i64,
    pub momentum: Momentum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Params {
    pub product: Product,
    pub strategy_stop: Option<i64>,
    pub strategy_target: Option<i64>,
    pub stop_premium_pct: Option<i64>,
    pub target_premium_pct: Option<i64>,
    pub lock_arm: Option<i64>,
    pub lock_floor: Option<i64>,
    pub lock_trails: bool,
    pub overall_trail_every: Option<i64>,
    pub overall_trail_by: Option<i64>,
    pub trail_distance: Option<PriceTicks>,
    pub wait_distance: Option<PriceTicks>,
    pub momentum: Momentum,
    pub monitor: Monitor,
    pub fill_window_ms: i64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            product: Product::Mis,
            strategy_stop: None,
            strategy_target: None,
            stop_premium_pct: None,
            target_premium_pct: None,
            lock_arm: None,
            lock_floor: None,
            lock_trails: false,
            overall_trail_every: None,
            overall_trail_by: None,
            trail_distance: None,
            wait_distance: None,
            momentum: Momentum::Off,
            monitor: Monitor::Candle { ms: 60_000 },
            fill_window_ms: 2_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegSpec {
    pub id: LegId,
    pub instrument: InstrumentId,
    pub side: Side,
    pub target_qty: QtyUnits,
    pub risk: LegRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Waiting,
    Entering,
    Live,
    Aborting,
    Halted,
    Exiting,
    Flat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegSnap {
    pub id: LegId,
    pub instrument: String,
    pub open: u64,
    pub closed: u64,
    pub entry: Option<i64>,
    pub exit: Option<i64>,
    pub trigger: Option<i64>,
    pub realized: i64,
    pub unrealized: i64,
}

#[derive(Debug, Clone)]
struct Leg {
    spec: LegSpec,
    entry_notional: i128,
    entry_qty: u64,
    exit_notional: i128,
    exit_qty: u64,
    open_qty: u64,
    realized: i64,
    mark: Option<PriceTicks>,
    fresh: bool,
    entry_id: Option<ClientOrderId>,
    stop_id: Option<ClientOrderId>,
    exit_id: Option<ClientOrderId>,
    stop_trigger: Option<PriceTicks>,
    stop_origin: Option<PriceTicks>,
    stop_acked: bool,
    entry_acked: bool,
    sent_ms: Option<i64>,
    ref_price: Option<PriceTicks>,
    best: Option<PriceTicks>,
    last_desired: Option<PriceTicks>,
    last_fill: Option<(u64, i64)>,
}

#[derive(Debug, Clone)]
pub struct Book {
    id: StrategyId,
    params: Params,
    legs: Vec<Leg>,
    phase: Phase,
    seq: u64,
    lock_armed: bool,
    best_pnl: i64,
    stop_origin: Option<i64>,
    lock_floor_origin: Option<i64>,
    candle_origin_ms: Option<i64>,
    candle_last_pnl: i64,
    ids: HashMap<String, usize>,
}

impl Book {
    pub fn new(id: impl Into<String>, params: Params, specs: Vec<LegSpec>) -> Self {
        let legs = specs
            .into_iter()
            .map(|spec| Leg {
                spec,
                entry_notional: 0,
                entry_qty: 0,
                exit_notional: 0,
                exit_qty: 0,
                open_qty: 0,
                realized: 0,
                mark: None,
                fresh: false,
                entry_id: None,
                stop_id: None,
                exit_id: None,
                stop_trigger: None,
                stop_origin: None,
                stop_acked: false,
                entry_acked: false,
                sent_ms: None,
                ref_price: None,
                best: None,
                last_desired: None,
                last_fill: None,
            })
            .collect();
        let stop_origin = params.strategy_stop;
        let lock_floor_origin = params.lock_floor;
        Self {
            id: StrategyId(id.into()),
            params,
            legs,
            phase: Phase::Waiting,
            seq: 0,
            lock_armed: false,
            best_pnl: 0,
            stop_origin,
            lock_floor_origin,
            candle_origin_ms: None,
            candle_last_pnl: 0,
            ids: HashMap::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id.0
    }

    pub fn stop(&self) -> Option<i64> {
        self.params.strategy_stop
    }

    pub fn monitor_label(&self) -> String {
        match self.params.monitor {
            Monitor::Ltp => "LTP".to_string(),
            Monitor::Candle { ms } if ms >= 180_000 => "Candle close 3m".to_string(),
            Monitor::Candle { .. } => "Candle close 1m".to_string(),
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn product(&self) -> Product {
        self.params.product
    }

    pub fn combined_pnl(&self) -> i64 {
        self.legs
            .iter()
            .map(|leg| leg.realized + leg.unrealized())
            .sum()
    }

    pub fn legs(&self) -> Vec<LegSnap> {
        self.legs
            .iter()
            .map(|leg| LegSnap {
                id: leg.spec.id,
                instrument: leg.spec.instrument.0.clone(),
                open: leg.open_qty,
                closed: leg.exit_qty,
                entry: leg.entry_price(),
                exit: leg.exit_price(),
                trigger: leg.stop_trigger.map(|price| price.get()),
                realized: leg.realized,
                unrealized: leg.unrealized(),
            })
            .collect()
    }

    pub fn resume_aborting(
        id: impl Into<String>,
        params: Params,
        specs: Vec<LegSpec>,
    ) -> (Self, Vec<Intent>) {
        let mut book = Self::new(id, params, specs);
        book.phase = Phase::Aborting;
        for leg in &mut book.legs {
            leg.open_qty = leg.spec.target_qty.get();
            leg.entry_qty = leg.open_qty;
            leg.entry_notional = i128::from(leg.open_qty);
            leg.fresh = true;
        }
        let intents = book.flatten();
        (book, intents)
    }

    pub fn activate(&mut self, now_ms: i64) -> Vec<Intent> {
        if self.params.wait_distance.is_some()
            || self.params.momentum != Momentum::Off
            || self
                .legs
                .iter()
                .any(|leg| leg.spec.risk.momentum != Momentum::Off)
        {
            self.phase = Phase::Waiting;
            if self.params.wait_distance.is_some() || self.params.momentum != Momentum::Off {
                return Vec::new();
            }
            return self.enter_unconditional(now_ms);
        }
        self.enter_all(now_ms)
    }

    pub fn on_tick(
        &mut self,
        instrument: &InstrumentId,
        price: PriceTicks,
        now_ms: i64,
    ) -> Vec<Intent> {
        if self.phase == Phase::Waiting && self.params.wait_distance.is_some() {
            return self.maybe_wait_fire(instrument, price, now_ms);
        }
        let mut intents = Vec::new();
        intents.extend(self.momentum_entry(instrument, price, now_ms));
        let live = self.phase == Phase::Live;
        let distance = self.params.trail_distance;
        let mut seq = self.seq;
        for leg in &mut self.legs {
            if &leg.spec.instrument != instrument || leg.open_qty == 0 {
                continue;
            }
            leg.mark = Some(price);
            leg.fresh = true;
            leg.note_best(price);
            if live {
                if let Some(modify) = leg.trail_intent(&mut seq, distance) {
                    intents.push(modify);
                }
            }
        }
        self.seq = seq;
        self.candle_last_pnl = self.combined_pnl();
        if self.candle_origin_ms.is_none() {
            self.candle_origin_ms = Some(now_ms);
        }
        if matches!(self.params.monitor, Monitor::Ltp) {
            intents.extend(self.evaluate_strategy());
        }
        intents
    }

    pub fn set_stale(&mut self, instrument: &InstrumentId) {
        for leg in &mut self.legs {
            if &leg.spec.instrument == instrument && leg.open_qty > 0 {
                leg.fresh = false;
            }
        }
    }

    pub fn on_event(&mut self, event: &OrderEvent) -> Vec<Intent> {
        match event {
            OrderEvent::Ack { client_id, .. } => self.on_ack(client_id),
            OrderEvent::Reject { client_id, reason } => self.on_reject(client_id, *reason),
            OrderEvent::Fill {
                client_id,
                qty,
                price,
            } => self.on_fill(client_id, *qty, *price),
            OrderEvent::Replaced { client_id, trigger } => {
                if let Some(leg) = self.leg_mut(client_id) {
                    leg.stop_trigger = Some(*trigger);
                }
                Vec::new()
            }
            OrderEvent::Canceled { .. } => Vec::new(),
        }
    }

    pub fn on_time(&mut self, now_ms: i64) -> Vec<Intent> {
        let mut intents = Vec::new();
        if self.phase == Phase::Entering {
            let window = self.params.fill_window_ms;
            let timed_out = self.legs.iter().any(|leg| {
                leg.sent_ms
                    .is_some_and(|sent| now_ms.saturating_sub(sent) >= window)
                    && (leg.entry_qty < leg.spec.target_qty.get() || !leg.entry_acked)
            });
            if timed_out {
                intents.extend(self.flatten());
            }
        }
        if let Monitor::Candle { ms } = self.params.monitor {
            if self.candle_origin_ms.is_none() {
                self.candle_origin_ms = Some(now_ms);
            }
            if let Some(origin) = self.candle_origin_ms {
                if now_ms >= origin.saturating_add(ms) && self.phase == Phase::Live {
                    self.candle_origin_ms = Some(origin.saturating_add(ms));
                    intents.extend(self.evaluate_strategy());
                }
            }
        }
        intents
    }

    pub fn square_off(&mut self) -> Vec<Intent> {
        if self.phase == Phase::Flat || self.phase == Phase::Waiting {
            return Vec::new();
        }
        self.phase = Phase::Exiting;
        self.flatten_keep_phase()
    }

    fn maybe_wait_fire(
        &mut self,
        instrument: &InstrumentId,
        price: PriceTicks,
        now_ms: i64,
    ) -> Vec<Intent> {
        let Some(distance) = self.params.wait_distance else {
            return Vec::new();
        };
        let ready = {
            let Some(leg) = self
                .legs
                .iter_mut()
                .find(|leg| &leg.spec.instrument == instrument)
            else {
                return Vec::new();
            };
            if leg.ref_price.is_none() {
                leg.ref_price = Some(price);
                return Vec::new();
            }
            leg.ref_price.unwrap().get().abs_diff(price.get()) >= distance.get().unsigned_abs()
        };
        if ready {
            self.enter_all(now_ms)
        } else {
            Vec::new()
        }
    }

    fn enter_unconditional(&mut self, now_ms: i64) -> Vec<Intent> {
        let ready: Vec<usize> = self
            .legs
            .iter()
            .enumerate()
            .filter(|(_, leg)| leg.entry_id.is_none() && leg.spec.risk.momentum == Momentum::Off)
            .map(|(index, _)| index)
            .collect();
        self.enter_indexes(ready, now_ms)
    }

    fn momentum_entry(
        &mut self,
        instrument: &InstrumentId,
        price: PriceTicks,
        now_ms: i64,
    ) -> Vec<Intent> {
        if matches!(
            self.phase,
            Phase::Flat | Phase::Exiting | Phase::Aborting | Phase::Halted
        ) {
            return Vec::new();
        }
        let Some(index) = self.legs.iter().position(|leg| {
            &leg.spec.instrument == instrument
                && leg.entry_id.is_none()
                && (leg.spec.risk.momentum != Momentum::Off
                    || self.params.momentum != Momentum::Off)
        }) else {
            return Vec::new();
        };
        let leg = &mut self.legs[index];
        if leg.ref_price.is_none() {
            leg.ref_price = Some(price);
            leg.mark = Some(price);
            return Vec::new();
        }
        leg.mark = Some(price);
        if !self.book_momentum_met() {
            return Vec::new();
        }
        let ready = self
            .legs
            .iter()
            .enumerate()
            .filter(|(_, leg)| leg.entry_id.is_none() && Self::leg_momentum_met(leg))
            .map(|(index, _)| index)
            .collect();
        self.enter_indexes(ready, now_ms)
    }

    fn leg_momentum_met(leg: &Leg) -> bool {
        let (Some(reference), Some(mark)) = (leg.ref_price, leg.mark) else {
            return false;
        };
        match leg.spec.risk.momentum {
            Momentum::Off => true,
            Momentum::PremiumUp(percent) => {
                mark.get() >= reference.get() + reference.get().saturating_mul(percent) / 100
            }
            Momentum::PremiumDown(percent) => {
                mark.get() <= reference.get() - reference.get().saturating_mul(percent) / 100
            }
            Momentum::PointsUp(_) | Momentum::PointsDown(_) => false,
        }
    }

    fn book_momentum_met(&self) -> bool {
        let start: i64 = self
            .legs
            .iter()
            .map(|leg| leg.ref_price.map(|price| price.get()).unwrap_or(0))
            .sum();
        let now: i64 = self
            .legs
            .iter()
            .map(|leg| leg.mark.map(|price| price.get()).unwrap_or(0))
            .sum();
        if self.params.momentum != Momentum::Off
            && self
                .legs
                .iter()
                .any(|leg| leg.ref_price.is_none() || leg.mark.is_none())
        {
            return false;
        }
        match self.params.momentum {
            Momentum::Off => true,
            Momentum::PremiumUp(percent) => now >= start + start.saturating_mul(percent) / 100,
            Momentum::PremiumDown(percent) => now <= start - start.saturating_mul(percent) / 100,
            Momentum::PointsUp(ticks) => now >= start.saturating_add(ticks),
            Momentum::PointsDown(ticks) => now <= start.saturating_sub(ticks),
        }
    }

    fn enter_indexes(&mut self, indexes: Vec<usize>, now_ms: i64) -> Vec<Intent> {
        if indexes.is_empty() {
            return Vec::new();
        }
        if self.phase == Phase::Waiting {
            self.phase = Phase::Entering;
        }
        let mut intents = Vec::new();
        for index in indexes {
            if self.legs[index].entry_id.is_some() {
                continue;
            }
            intents.push(self.place_entry(index, now_ms));
        }
        intents
    }

    fn place_entry(&mut self, index: usize, now_ms: i64) -> Intent {
        let intent = self.place(
            index,
            OrderRole::Entry,
            self.legs[index].spec.target_qty,
            None,
        );
        if let Intent::Place(place) = &intent {
            let id = place.client_id.0.clone();
            self.legs[index].entry_id = Some(place.client_id.clone());
            self.legs[index].sent_ms = Some(now_ms);
            self.ids.insert(id, index);
        }
        intent
    }

    fn enter_all(&mut self, now_ms: i64) -> Vec<Intent> {
        self.phase = Phase::Entering;
        let mut intents = Vec::new();
        for index in 0..self.legs.len() {
            if self.legs[index].entry_id.is_some() {
                continue;
            }
            intents.push(self.place_entry(index, now_ms));
        }
        intents
    }

    fn on_ack(&mut self, client_id: &ClientOrderId) -> Vec<Intent> {
        let Some(index) = self.ids.get(&client_id.0).copied() else {
            return Vec::new();
        };
        if self.legs[index].stop_id.as_ref() == Some(client_id) {
            self.legs[index].stop_acked = true;
            self.promote_live();
        } else if self.legs[index].entry_id.as_ref() == Some(client_id) {
            self.legs[index].entry_acked = true;
        }
        Vec::new()
    }

    fn on_reject(
        &mut self,
        client_id: &ClientOrderId,
        _reason: broker_api::RejectReason,
    ) -> Vec<Intent> {
        if self.phase != Phase::Entering && self.phase != Phase::Waiting {
            return Vec::new();
        }
        let _ = client_id;
        self.flatten()
    }

    fn on_fill(
        &mut self,
        client_id: &ClientOrderId,
        qty: QtyUnits,
        price: PriceTicks,
    ) -> Vec<Intent> {
        let Some(index) = self.ids.get(&client_id.0).copied() else {
            return Vec::new();
        };
        let fingerprint = (qty.get(), price.get());
        if self.legs[index].last_fill == Some(fingerprint) {
            return Vec::new();
        }
        self.legs[index].last_fill = Some(fingerprint);
        let is_exit = self.legs[index].exit_id.as_ref() == Some(client_id)
            || self.legs[index].stop_id.as_ref() == Some(client_id);
        if is_exit {
            self.legs[index].apply_exit(price, qty.get());
        } else {
            self.legs[index].apply_entry(price, qty.get());
        }
        let mut intents = Vec::new();
        if self.phase == Phase::Entering && self.entries_complete() {
            intents.extend(self.place_stops());
        }
        if self.open_qty() == 0
            && matches!(
                self.phase,
                Phase::Aborting | Phase::Exiting | Phase::Entering
            )
        {
            self.phase = Phase::Flat;
        }
        self.promote_live();
        intents
    }

    fn place_stops(&mut self) -> Vec<Intent> {
        let mut intents = Vec::new();
        for index in 0..self.legs.len() {
            if self.legs[index].open_qty == 0 || self.legs[index].stop_id.is_some() {
                continue;
            }
            let trigger = self.legs[index].stop_from_entry();
            let qty = QtyUnits::new(self.legs[index].open_qty);
            let intent = self.place(index, OrderRole::Stop, qty, trigger);
            if let Intent::Place(place) = &intent {
                self.legs[index].stop_id = Some(place.client_id.clone());
                self.legs[index].stop_trigger = trigger;
                self.legs[index].stop_origin = trigger;
                self.ids.insert(place.client_id.0.clone(), index);
            }
            intents.push(intent);
        }
        self.apply_premium_limits();
        intents
    }

    fn promote_live(&mut self) {
        if self.phase == Phase::Entering
            && self.entries_complete()
            && self
                .legs
                .iter()
                .filter(|leg| leg.open_qty > 0)
                .all(|leg| leg.stop_acked)
        {
            self.phase = Phase::Live;
        }
    }

    fn entries_complete(&self) -> bool {
        self.legs
            .iter()
            .all(|leg| leg.entry_qty >= leg.spec.target_qty.get())
    }

    fn evaluate_strategy(&mut self) -> Vec<Intent> {
        if self.phase != Phase::Live {
            return Vec::new();
        }
        if self.legs.iter().any(|leg| leg.open_qty > 0 && !leg.fresh) {
            return Vec::new();
        }
        let pnl = self.combined_pnl();
        if pnl > self.best_pnl {
            self.best_pnl = pnl;
        }
        self.lift_overall_stop();
        if let (Some(arm), Some(_)) = (self.params.lock_arm, self.params.lock_floor) {
            if pnl >= arm {
                self.lock_armed = true;
            }
        }
        self.lift_lock();
        let stop_hit = self.params.strategy_stop.is_some_and(|stop| pnl <= stop);
        let target_hit = self
            .params
            .strategy_target
            .is_some_and(|target| pnl >= target);
        let lock_hit = self.lock_armed && self.params.lock_floor.is_some_and(|floor| pnl <= floor);
        if stop_hit || lock_hit || target_hit {
            self.phase = Phase::Exiting;
            return self.flatten_keep_phase();
        }
        Vec::new()
    }

    fn apply_premium_limits(&mut self) {
        let premium: i64 = self
            .legs
            .iter()
            .filter_map(|leg| {
                let entry = leg.entry_price()?;
                Some(entry.saturating_mul(i64::try_from(leg.entry_qty).unwrap_or(0)))
            })
            .sum();
        if premium == 0 {
            return;
        }
        if let Some(pct) = self.params.stop_premium_pct {
            let stop = -premium.saturating_mul(pct) / 100;
            self.params.strategy_stop = Some(stop);
            self.stop_origin = Some(stop);
        }
        if let Some(pct) = self.params.target_premium_pct {
            self.params.strategy_target = Some(premium.saturating_mul(pct) / 100);
        }
    }

    fn lift_overall_stop(&mut self) {
        if self.params.lock_trails {
            return;
        }
        let (Some(every), Some(by)) = (
            self.params.overall_trail_every,
            self.params.overall_trail_by,
        ) else {
            return;
        };
        if every <= 0 || by <= 0 {
            return;
        }
        let Some(origin) = self.stop_origin else {
            return;
        };
        let steps = self.best_pnl.max(0) / every;
        let lifted = origin.saturating_add(steps.saturating_mul(by));
        if self.params.strategy_stop.is_none_or(|stop| lifted > stop) {
            self.params.strategy_stop = Some(lifted);
        }
    }

    fn lift_lock(&mut self) {
        if !self.params.lock_trails || !self.lock_armed {
            return;
        }
        let (Some(every), Some(by)) = (
            self.params.overall_trail_every,
            self.params.overall_trail_by,
        ) else {
            return;
        };
        if every <= 0 || by <= 0 {
            return;
        }
        let Some(arm) = self.params.lock_arm else {
            return;
        };
        let Some(origin) = self.lock_floor_origin else {
            return;
        };
        let steps = self.best_pnl.saturating_sub(arm).max(0) / every;
        let lifted = origin.saturating_add(steps.saturating_mul(by));
        if self.params.lock_floor.is_none_or(|floor| lifted > floor) {
            self.params.lock_floor = Some(lifted);
        }
    }

    fn flatten(&mut self) -> Vec<Intent> {
        self.phase = Phase::Aborting;
        self.flatten_keep_phase()
    }

    fn flatten_keep_phase(&mut self) -> Vec<Intent> {
        let mut intents = Vec::new();
        for index in 0..self.legs.len() {
            let working = self.legs[index].entry_id.clone();
            let incomplete = self.legs[index].entry_qty < self.legs[index].spec.target_qty.get();
            if incomplete {
                if let Some(client_id) = working {
                    intents.push(Intent::Cancel { client_id });
                }
            }
            if self.legs[index].open_qty > 0 && self.legs[index].exit_id.is_none() {
                let qty = QtyUnits::new(self.legs[index].open_qty);
                let intent = self.place(index, OrderRole::Exit, qty, None);
                if let Intent::Place(place) = &intent {
                    self.legs[index].exit_id = Some(place.client_id.clone());
                    self.ids.insert(place.client_id.0.clone(), index);
                }
                intents.push(intent);
            }
        }
        if self.open_qty() == 0 {
            self.phase = Phase::Flat;
        }
        intents
    }

    fn open_qty(&self) -> u64 {
        self.legs.iter().map(|leg| leg.open_qty).sum()
    }

    fn place(
        &mut self,
        index: usize,
        role: OrderRole,
        qty: QtyUnits,
        trigger: Option<PriceTicks>,
    ) -> Intent {
        self.seq += 1;
        let leg = &self.legs[index];
        let side = match role {
            OrderRole::Entry => leg.spec.side,
            OrderRole::Stop | OrderRole::Exit => match leg.spec.side {
                Side::Buy => Side::Sell,
                Side::Sell => Side::Buy,
            },
        };
        Intent::Place(PlaceIntent {
            client_id: ClientOrderId(format!(
                "{}-{}-{}-{}",
                self.id.0,
                leg.spec.id.0,
                role_tag(role),
                self.seq
            )),
            strategy_id: self.id.clone(),
            leg_id: leg.spec.id,
            instrument: leg.spec.instrument.clone(),
            side,
            qty,
            role,
            trigger,
        })
    }

    fn leg_mut(&mut self, client_id: &ClientOrderId) -> Option<&mut Leg> {
        let index = *self.ids.get(&client_id.0)?;
        self.legs.get_mut(index)
    }
}

impl Leg {
    fn entry_price(&self) -> Option<i64> {
        average(self.entry_notional, self.entry_qty)
    }

    fn exit_price(&self) -> Option<i64> {
        average(self.exit_notional, self.exit_qty)
    }

    fn unrealized(&self) -> i64 {
        if self.open_qty == 0 {
            return 0;
        }
        let Some(mark) = self.mark else {
            return 0;
        };
        let Some(entry) = self.entry_price() else {
            return 0;
        };
        tradecore::signed_pnl(
            PriceTicks::new(entry),
            mark,
            QtyUnits::new(self.open_qty),
            self.spec.side.is_sell(),
        )
    }

    fn apply_entry(&mut self, price: PriceTicks, qty: u64) {
        let room = self.spec.target_qty.get().saturating_sub(self.entry_qty);
        let qty = qty.min(room);
        if qty == 0 {
            return;
        }
        self.entry_notional += i128::from(price.get()) * i128::from(qty);
        self.entry_qty += qty;
        self.open_qty += qty;
        self.mark = Some(price);
        self.fresh = true;
        self.best = Some(price);
    }

    fn apply_exit(&mut self, price: PriceTicks, qty: u64) {
        let qty = qty.min(self.open_qty);
        if qty == 0 {
            return;
        }
        let Some(entry) = self.entry_price() else {
            return;
        };
        self.realized += tradecore::signed_pnl(
            PriceTicks::new(entry),
            price,
            QtyUnits::new(qty),
            self.spec.side.is_sell(),
        );
        self.exit_notional += i128::from(price.get()) * i128::from(qty);
        self.exit_qty += qty;
        self.open_qty -= qty;
        if self.open_qty == 0 {
            self.mark = None;
            self.fresh = false;
        }
    }

    fn note_best(&mut self, price: PriceTicks) {
        self.best = Some(match self.best {
            Some(best) if self.spec.side.is_sell() => PriceTicks::new(best.get().min(price.get())),
            Some(best) => PriceTicks::new(best.get().max(price.get())),
            None => price,
        });
    }

    fn trail_intent(&mut self, seq: &mut u64, distance: Option<PriceTicks>) -> Option<Intent> {
        if self.spec.risk.trail != TrailMode::Off {
            let desired = self.stepped_trail()?;
            return self.emit_trail(seq, desired);
        }
        let distance = distance?;
        let best = self.best?;
        let desired = if self.spec.side.is_sell() {
            PriceTicks::new(best.get().saturating_add(distance.get()))
        } else {
            PriceTicks::new(best.get().saturating_sub(distance.get()))
        };
        self.emit_trail(seq, desired)
    }

    fn stepped_trail(&self) -> Option<PriceTicks> {
        let entry = self.entry_price()?;
        let best = self.best?;
        let origin = self.stop_origin?;
        let (every, by) = match self.spec.risk.trail {
            TrailMode::Points => (self.spec.risk.trail_every, self.spec.risk.trail_by),
            TrailMode::Percent => (
                entry.saturating_mul(self.spec.risk.trail_every) / 100,
                entry.saturating_mul(self.spec.risk.trail_by) / 100,
            ),
            TrailMode::Off => return None,
        };
        if every <= 0 || by <= 0 {
            return None;
        }
        let favorable = if self.spec.side.is_sell() {
            entry.saturating_sub(best.get())
        } else {
            best.get().saturating_sub(entry)
        };
        if favorable <= 0 {
            return None;
        }
        let steps = favorable / every;
        if steps == 0 {
            return None;
        }
        let shift = steps.saturating_mul(by);
        let desired = if self.spec.side.is_sell() {
            origin.get().saturating_sub(shift)
        } else {
            origin.get().saturating_add(shift)
        };
        Some(PriceTicks::new(desired))
    }

    fn emit_trail(&mut self, seq: &mut u64, desired: PriceTicks) -> Option<Intent> {
        let stop_id = self.stop_id.clone()?;
        let tighter = match self.stop_trigger {
            Some(current) if self.spec.side.is_sell() => desired.get() < current.get(),
            Some(current) => desired.get() > current.get(),
            None => true,
        };
        if !tighter || self.last_desired == Some(desired) {
            return None;
        }
        self.last_desired = Some(desired);
        *seq += 1;
        Some(Intent::Modify {
            client_id: stop_id,
            trigger: desired,
            priority: IntentPriority::Trail,
        })
    }

    fn stop_from_entry(&self) -> Option<PriceTicks> {
        let entry = self.entry_price()?;
        let offset = match self.spec.risk.sl {
            SlMode::Off | SlMode::UnderlyingPoints | SlMode::UnderlyingPercent => 0,
            SlMode::Points => self.spec.risk.sl_value,
            SlMode::Percent => entry.saturating_mul(self.spec.risk.sl_value) / 100,
        };
        let trigger = if offset == 0 {
            entry
        } else if self.spec.side.is_sell() {
            entry.saturating_add(offset)
        } else {
            entry.saturating_sub(offset)
        };
        Some(PriceTicks::new(trigger))
    }
}

fn average(notional: i128, qty: u64) -> Option<i64> {
    if qty == 0 {
        None
    } else {
        i64::try_from(notional / i128::from(qty)).ok()
    }
}

fn role_tag(role: OrderRole) -> &'static str {
    match role {
        OrderRole::Entry => "entry",
        OrderRole::Stop => "stop",
        OrderRole::Exit => "exit",
    }
}
