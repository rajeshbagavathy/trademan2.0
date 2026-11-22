use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use broker_api::{InstrumentId, Intent, LegId, OrderRole, Side};
use serde::{Deserialize, Serialize};
use strategy::{Book, LegSpec, Params, Phase};
use tower_http::cors::CorsLayer;
use tradecore::{PriceTicks, QtyUnits};

mod definition;

use definition::{materialize, Definition, Execution};

#[derive(Clone)]
struct AppState {
    desk: Arc<Mutex<Desk>>,
}

struct Desk {
    books: Vec<Book>,
    meta: HashMap<String, Meta>,
    orders: Vec<OrderRow>,
    fills: Vec<FillRow>,
    audit: Vec<AuditRow>,
    offset_ms: i64,
}

struct Meta {
    broker: String,
    paused: bool,
    definition: Definition,
    queued: bool,
    starts_at: Option<i64>,
    started_at: Option<i64>,
}

#[derive(Serialize)]
struct Snapshot {
    mode: &'static str,
    feeds: Vec<FeedRow>,
    strategies: Vec<StrategyRow>,
    orders: Vec<OrderRow>,
    fills: Vec<FillRow>,
    audit: Vec<AuditRow>,
}

#[derive(Serialize, Clone)]
struct FeedRow {
    name: &'static str,
    role: &'static str,
}

#[derive(Serialize)]
struct StrategyRow {
    id: String,
    name: String,
    broker: String,
    state: String,
    monitor: String,
    combined: i64,
    realized: i64,
    unrealized: i64,
    stop: Option<i64>,
    product: String,
    starts_at: Option<i64>,
    started_at: Option<i64>,
    legs: Vec<LegRow>,
    definition: Definition,
}

#[derive(Serialize)]
struct LegRow {
    id: u32,
    symbol: String,
    side: String,
    open: u64,
    closed: u64,
    entry: Option<i64>,
    exit: Option<i64>,
    trigger: Option<i64>,
    realized: i64,
    unrealized: i64,
}

#[derive(Serialize, Clone)]
struct OrderRow {
    strategy: String,
    leg: String,
    role: String,
    state: String,
    trigger: Option<i64>,
    traded: Option<i64>,
    qty: u64,
}

#[derive(Serialize, Clone)]
struct FillRow {
    strategy: String,
    leg: String,
    side: String,
    qty: u64,
    price: i64,
}

#[derive(Serialize, Clone)]
struct AuditRow {
    source: String,
    event: String,
}

#[derive(Deserialize, Default)]
struct ActivateBody {
    broker: Option<String>,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        desk: Arc::new(Mutex::new(seed())),
    };
    let app = Router::new()
        .route("/api/snapshot", get(snapshot))
        .route("/api/strategies", post(add_strategy))
        .route("/api/strategies/activate-inactive", post(activate_inactive))
        .route("/api/strategies/{id}/activate", post(activate))
        .route("/api/strategies/{id}/execution", post(edit_execution))
        .route("/api/strategies/{id}/pause", post(pause))
        .route("/api/strategies/{id}/resume", post(resume))
        .route("/api/strategies/{id}/square-off", post(square_off))
        .layer(CorsLayer::permissive())
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8090")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn snapshot(State(state): State<AppState>) -> Json<Snapshot> {
    let mut desk = state.desk.lock().expect("desk");
    let now = utc_now_ms();
    desk.release_due(now);
    Json(desk.view())
}

async fn add_strategy(
    State(state): State<AppState>,
    Json(body): Json<Definition>,
) -> Json<Snapshot> {
    let mut desk = state.desk.lock().expect("desk");
    desk.add(body);
    Json(desk.view())
}

async fn activate_inactive(State(state): State<AppState>) -> Json<Snapshot> {
    let mut desk = state.desk.lock().expect("desk");
    let ids: Vec<_> = desk
        .books
        .iter()
        .filter(|book| book.phase() == Phase::Waiting)
        .map(book_id)
        .collect();
    for id in ids {
        desk.activate(&id, None);
    }
    Json(desk.view())
}

async fn activate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ActivateBody>,
) -> Json<Snapshot> {
    let mut desk = state.desk.lock().expect("desk");
    desk.activate(&id, body.broker);
    Json(desk.view())
}

async fn edit_execution(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Execution>,
) -> Json<Snapshot> {
    let mut desk = state.desk.lock().expect("desk");
    desk.set_execution(&id, body);
    Json(desk.view())
}

async fn pause(State(state): State<AppState>, Path(id): Path<String>) -> Json<Snapshot> {
    let mut desk = state.desk.lock().expect("desk");
    if let Some(meta) = desk.meta.get_mut(&id) {
        meta.paused = true;
        desk.audit.push(AuditRow {
            source: "UI".into(),
            event: format!("{id} paused"),
        });
    }
    Json(desk.view())
}

async fn resume(State(state): State<AppState>, Path(id): Path<String>) -> Json<Snapshot> {
    let mut desk = state.desk.lock().expect("desk");
    if let Some(meta) = desk.meta.get_mut(&id) {
        meta.paused = false;
        desk.audit.push(AuditRow {
            source: "UI".into(),
            event: format!("{id} resumed"),
        });
    }
    Json(desk.view())
}

async fn square_off(State(state): State<AppState>, Path(id): Path<String>) -> Json<Snapshot> {
    let mut desk = state.desk.lock().expect("desk");
    if desk.cancel_queue(&id) {
        desk.audit.push(AuditRow {
            source: "UI".into(),
            event: format!("{id} removed from the queue"),
        });
    } else if let Some(book) = desk.books.iter_mut().find(|book| book_id(book) == id) {
        let intents = book.square_off();
        let name = id.clone();
        desk.record(&name, &intents);
        desk.audit.push(AuditRow {
            source: "UI".into(),
            event: format!("{id} square off sent to the engine"),
        });
    }
    Json(desk.view())
}

impl Desk {
    fn add(&mut self, definition: Definition) {
        let mut id = slug(&definition.name);
        if id.is_empty() {
            id = "strategy".into();
        }
        if self.meta.contains_key(&id) {
            id = format!("{id}-{}", self.books.len() + 1);
        }
        self.books.push(materialize(&id, &definition));
        self.meta.insert(
            id.clone(),
            Meta {
                broker: "Not activated".into(),
                paused: false,
                queued: false,
                starts_at: None,
                started_at: None,
                definition,
            },
        );
        self.audit.push(AuditRow {
            source: "UI".into(),
            event: format!("{id} saved inactive"),
        });
    }

    fn set_execution(&mut self, id: &str, execution: Execution) {
        let Some(meta) = self.meta.get_mut(id) else {
            return;
        };
        meta.definition.execution = execution;
        let definition = meta.definition.clone();
        let waiting = self
            .books
            .iter()
            .find(|book| book_id(book) == id)
            .is_some_and(|book| book.phase() == Phase::Waiting);
        if waiting {
            if let Some(index) = self.books.iter().position(|book| book_id(book) == id) {
                self.books[index] = materialize(id, &definition);
            }
        }
        self.audit.push(AuditRow {
            source: "UI".into(),
            event: format!("{id} execution settings saved"),
        });
    }

    fn activate(&mut self, id: &str, broker: Option<String>) {
        self.activate_at(id, broker, utc_now_ms());
    }

    fn activate_at(&mut self, id: &str, broker: Option<String>, now_ms: i64) {
        let Some(index) = self.books.iter().position(|book| book_id(book) == id) else {
            return;
        };
        if self.books[index].phase() != Phase::Waiting
            || self.meta.get(id).is_some_and(|meta| meta.queued)
        {
            return;
        }
        let entry = self
            .meta
            .get(id)
            .map(|meta| meta.definition.entry_time.clone())
            .unwrap_or_else(|| "09:20".into());
        let starts = entry_start_ms(now_ms, self.offset_ms, &entry);
        if let Some(broker) = broker.filter(|broker| !broker.is_empty()) {
            if let Some(meta) = self.meta.get_mut(id) {
                meta.broker = broker;
            }
        }
        if starts > now_ms {
            if let Some(meta) = self.meta.get_mut(id) {
                meta.queued = true;
                meta.starts_at = Some(starts);
                meta.paused = false;
            }
            self.audit.push(AuditRow {
                source: "UI".into(),
                event: format!("{id} queued for {entry}"),
            });
            return;
        }
        if let Some(meta) = self.meta.get_mut(id) {
            meta.starts_at = Some(starts);
        }
        self.enter_now(id, now_ms);
    }

    fn cancel_queue(&mut self, id: &str) -> bool {
        let waiting = self
            .books
            .iter()
            .any(|book| book_id(book) == id && book.phase() == Phase::Waiting);
        if !waiting {
            return false;
        }
        if let Some(meta) = self.meta.get_mut(id) {
            meta.queued = false;
            meta.paused = false;
            meta.starts_at = None;
        }
        true
    }

    fn release_due(&mut self, now_ms: i64) {
        let due: Vec<String> = self
            .meta
            .iter()
            .filter(|(_, meta)| {
                meta.queued && meta.starts_at.is_some_and(|starts| starts <= now_ms) && !meta.paused
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            self.enter_now(&id, now_ms);
        }
    }

    fn enter_now(&mut self, id: &str, now_ms: i64) {
        let Some(index) = self.books.iter().position(|book| book_id(book) == id) else {
            return;
        };
        if self.books[index].phase() != Phase::Waiting {
            return;
        }
        let legs = self.books[index].legs().len();
        let intents = self.books[index].activate(0);
        let sent = intents
            .iter()
            .filter(
                |intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Entry),
            )
            .count();
        self.record(id, &intents);
        if sent < legs {
            self.audit.push(AuditRow {
                source: "Engine".into(),
                event: format!(
                    "{id} held {} entry until the selected premium move",
                    legs - sent
                ),
            });
        }
        if let Some(meta) = self.meta.get_mut(id) {
            meta.paused = false;
            meta.queued = false;
            meta.started_at = Some(now_ms);
        }
        let broker = self
            .meta
            .get(id)
            .map(|meta| meta.broker.clone())
            .unwrap_or_else(|| "the broker".into());
        self.audit.push(AuditRow {
            source: "UI".into(),
            event: format!("{id} activated on {broker}. The engine owns the book."),
        });
    }

    fn record(&mut self, strategy: &str, intents: &[Intent]) {
        for intent in intents {
            if let Intent::Place(place) = intent {
                self.orders.push(OrderRow {
                    strategy: strategy.to_string(),
                    leg: place.instrument.0.clone(),
                    role: format!("{:?}", place.role).to_lowercase(),
                    state: "sent".into(),
                    trigger: place.trigger.map(|price| price.get()),
                    traded: None,
                    qty: place.qty.get(),
                });
            }
        }
    }

    fn view(&self) -> Snapshot {
        Snapshot {
            mode: "paper",
            feeds: vec![
                FeedRow {
                    name: "Firstock",
                    role: "primary",
                },
                FeedRow {
                    name: "5paisa",
                    role: "backup",
                },
            ],
            strategies: self
                .books
                .iter()
                .map(|book| {
                    let id = book_id(book);
                    let meta = self.meta.get(&id);
                    let realized = book.legs().iter().map(|leg| leg.realized).sum();
                    let unrealized = book.legs().iter().map(|leg| leg.unrealized).sum();
                    StrategyRow {
                        name: id.clone(),
                        broker: meta
                            .map(|meta| meta.broker.clone())
                            .unwrap_or_else(|| "Firstock".into()),
                        state: if meta.is_some_and(|meta| meta.paused) {
                            "Paused".into()
                        } else if meta.is_some_and(|meta| meta.queued)
                            && book.phase() == Phase::Waiting
                        {
                            "Queued".into()
                        } else {
                            format!("{:?}", book.phase())
                        },
                        starts_at: meta.and_then(|meta| meta.starts_at),
                        started_at: meta.and_then(|meta| meta.started_at),
                        monitor: book.monitor_label(),
                        combined: book.combined_pnl(),
                        realized,
                        unrealized,
                        stop: book.stop(),
                        product: format!("{:?}", book.product()),
                        definition: meta
                            .map(|meta| meta.definition.clone())
                            .unwrap_or_else(|| Definition::sample(&id, "NIFTY", 0.0, 1)),
                        legs: book
                            .legs()
                            .into_iter()
                            .map(|leg| LegRow {
                                id: leg.id.0,
                                symbol: leg.instrument,
                                side: String::new(),
                                open: leg.open,
                                closed: leg.closed,
                                entry: leg.entry,
                                exit: leg.exit,
                                trigger: leg.trigger,
                                realized: leg.realized,
                                unrealized: leg.unrealized,
                            })
                            .collect(),
                        id,
                    }
                })
                .collect(),
            orders: self.orders.clone(),
            fills: self.fills.clone(),
            audit: self.audit.clone(),
        }
    }
}

fn utc_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn local_offset_ms() -> i64 {
    #[cfg(windows)]
    {
        windows_local_offset_ms()
    }
    #[cfg(unix)]
    {
        unix_local_offset_ms()
    }
    #[cfg(not(any(windows, unix)))]
    {
        0
    }
}

#[cfg(windows)]
fn windows_local_offset_ms() -> i64 {
    #[repr(C)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemTime(time: *mut SystemTime);
        fn GetLocalTime(time: *mut SystemTime);
        fn SystemTimeToFileTime(time: *const SystemTime, file_time: *mut u64) -> i32;
    }
    unsafe {
        let mut utc = std::mem::zeroed();
        let mut local = std::mem::zeroed();
        GetSystemTime(&mut utc);
        GetLocalTime(&mut local);
        let mut utc_file = 0u64;
        let mut local_file = 0u64;
        SystemTimeToFileTime(&utc, &mut utc_file);
        SystemTimeToFileTime(&local, &mut local_file);
        (local_file as i64 - utc_file as i64) / 10_000
    }
}

/// Local offset from UTC, in milliseconds. Uses the POSIX clock so a Mac and a
/// Windows machine in the same timezone queue an entry at the same wall time.
#[cfg(unix)]
fn unix_local_offset_ms() -> i64 {
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const i8,
    }
    unsafe extern "C" {
        fn time(tloc: *mut i64) -> i64;
        fn gmtime_r(timer: *const i64, result: *mut Tm) -> *mut Tm;
        fn mktime(time: *mut Tm) -> i64;
    }
    unsafe {
        let now = time(std::ptr::null_mut());
        if now < 0 {
            return 0;
        }
        let mut utc_fields = std::mem::zeroed();
        if gmtime_r(&now, &mut utc_fields).is_null() {
            return 0;
        }
        utc_fields.tm_isdst = -1;
        let interpreted_as_local = mktime(&mut utc_fields);
        if interpreted_as_local < 0 {
            return 0;
        }
        now.saturating_sub(interpreted_as_local)
            .saturating_mul(1000)
    }
}

fn entry_start_ms(now_ms: i64, offset_ms: i64, entry: &str) -> i64 {
    let Some((hour, minute)) = entry.split_once(':') else {
        return now_ms;
    };
    let Ok(hour) = hour.parse::<i64>() else {
        return now_ms;
    };
    let Ok(minute) = minute.parse::<i64>() else {
        return now_ms;
    };
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) {
        return now_ms;
    }
    let local = now_ms.saturating_add(offset_ms);
    let day = local.div_euclid(86_400_000) * 86_400_000;
    day + (hour * 60 + minute) * 60 * 1000 - offset_ms
}

fn book_id(book: &Book) -> String {
    book.id().to_string()
}

fn slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    slug.trim_matches('-').to_string()
}

fn seed() -> Desk {
    let params = Params {
        strategy_stop: Some(-200_000),
        ..Params::default()
    };
    let mut book = Book::new(
        "nifty-22500-strangle",
        params,
        vec![
            LegSpec {
                id: LegId(1),
                instrument: InstrumentId("22500 CE".into()),
                side: Side::Sell,
                target_qty: QtyUnits::new(65),
                risk: strategy::LegRisk::default(),
            },
            LegSpec {
                id: LegId(2),
                instrument: InstrumentId("22500 PE".into()),
                side: Side::Sell,
                target_qty: QtyUnits::new(65),
                risk: strategy::LegRisk::default(),
            },
        ],
    );
    let entries = book.activate(0);
    let mut stops = Vec::new();
    let mut fills = Vec::new();
    for intent in &entries {
        let Intent::Place(place) = intent else {
            continue;
        };
        book.on_event(&broker_api::OrderEvent::Ack {
            client_id: place.client_id.clone(),
            broker_id: broker_api::BrokerOrderId(place.client_id.0.clone()),
        });
        let price = if place.leg_id.0 == 1 { 14_250 } else { 12_800 };
        let produced = book.on_event(&broker_api::OrderEvent::Fill {
            client_id: place.client_id.clone(),
            qty: place.qty,
            price: PriceTicks::new(price),
        });
        fills.push(FillRow {
            strategy: "nifty-22500-strangle".into(),
            leg: place.instrument.0.clone(),
            side: "sell".into(),
            qty: place.qty.get(),
            price,
        });
        stops.extend(produced);
    }
    let mut orders = Vec::new();
    for intent in entries.iter().chain(stops.iter()) {
        let Intent::Place(place) = intent else {
            continue;
        };
        if place.role == OrderRole::Stop {
            book.on_event(&broker_api::OrderEvent::Ack {
                client_id: place.client_id.clone(),
                broker_id: broker_api::BrokerOrderId(place.client_id.0.clone()),
            });
        }
        orders.push(OrderRow {
            strategy: "nifty-22500-strangle".into(),
            leg: place.instrument.0.clone(),
            role: format!("{:?}", place.role).to_lowercase(),
            state: "acknowledged".into(),
            trigger: place.trigger.map(|price| price.get()),
            traded: None,
            qty: place.qty.get(),
        });
    }
    if let Some(Intent::Place(put_stop)) = stops
        .iter()
        .find(|intent| matches!(intent, Intent::Place(place) if place.leg_id.0 == 2))
    {
        book.on_event(&broker_api::OrderEvent::Fill {
            client_id: put_stop.client_id.clone(),
            qty: put_stop.qty,
            price: PriceTicks::new(13_385),
        });
        fills.push(FillRow {
            strategy: "nifty-22500-strangle".into(),
            leg: "22500 PE".into(),
            side: "buy".into(),
            qty: 65,
            price: 13_385,
        });
    }
    book.on_tick(
        &InstrumentId("22500 CE".into()),
        PriceTicks::new(11_800),
        1_000,
    );
    let bank = Params {
        strategy_stop: Some(-300_000),
        ..Params::default()
    };
    Desk {
        books: vec![
            book,
            Book::new(
                "banknifty-48000-straddle",
                bank,
                vec![
                    LegSpec {
                        id: LegId(1),
                        instrument: InstrumentId("48000 CE".into()),
                        side: Side::Sell,
                        target_qty: QtyUnits::new(30),
                        risk: strategy::LegRisk::default(),
                    },
                    LegSpec {
                        id: LegId(2),
                        instrument: InstrumentId("48000 PE".into()),
                        side: Side::Sell,
                        target_qty: QtyUnits::new(30),
                        risk: strategy::LegRisk::default(),
                    },
                ],
            ),
        ],
        offset_ms: local_offset_ms(),
        meta: HashMap::from([
            (
                "nifty-22500-strangle".into(),
                Meta {
                    broker: "Firstock".into(),
                    paused: false,
                    queued: false,
                    starts_at: Some(entry_start_ms(utc_now_ms(), local_offset_ms(), "09:20")),
                    started_at: Some(entry_start_ms(utc_now_ms(), local_offset_ms(), "09:20").min(utc_now_ms())),
                    definition: Definition::sample("nifty-22500-strangle", "NIFTY", 22500.0, 1),
                },
            ),
            (
                "banknifty-48000-straddle".into(),
                Meta {
                    broker: "5paisa".into(),
                    paused: false,
                    queued: false,
                    starts_at: None,
                    started_at: None,
                    definition: Definition::sample("banknifty-48000-straddle", "BANKNIFTY", 48000.0, 1),
                },
            ),
        ]),
        orders,
        fills,
        audit: vec![AuditRow {
            source: "Engine".into(),
            event: "Put closed at traded price 133.85. Combined P&L uses that exit, not the stop trigger.".into(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_before_the_entry_time_queues_without_an_order() {
        let mut desk = seed();
        desk.offset_ms = 0;
        let id = "banknifty-48000-straddle";
        desk.meta.get_mut(id).expect("bank").definition.entry_time = "15:00".into();
        let now = 86_400_000 + 14 * 3_600_000;
        desk.activate_at(id, Some("Firstock".into()), now);
        let snap = desk.view();
        let bank = snap
            .strategies
            .iter()
            .find(|row| row.id == id)
            .expect("bank");
        assert_eq!(bank.state, "Queued");
        assert_eq!(bank.starts_at, Some(86_400_000 + 15 * 3_600_000));
        assert!(snap.orders.iter().all(|order| order.strategy != id));
        desk.release_due(86_400_000 + 15 * 3_600_000);
        let snap = desk.view();
        let bank = snap
            .strategies
            .iter()
            .find(|row| row.id == id)
            .expect("bank");
        assert_eq!(bank.state, "Entering");
        assert_eq!(bank.started_at, Some(86_400_000 + 15 * 3_600_000));
        assert!(snap
            .orders
            .iter()
            .any(|order| order.strategy == id && order.role == "entry"));
    }

    #[test]
    fn square_off_before_the_entry_time_returns_the_strategy_to_saved() {
        let mut desk = seed();
        desk.offset_ms = 0;
        let id = "banknifty-48000-straddle";
        desk.meta.get_mut(id).expect("bank").definition.entry_time = "15:00".into();
        let now = 86_400_000 + 14 * 3_600_000;
        desk.activate_at(id, Some("Firstock".into()), now);
        assert!(desk.cancel_queue(id));
        desk.release_due(86_400_000 + 15 * 3_600_000);
        let snap = desk.view();
        let bank = snap
            .strategies
            .iter()
            .find(|row| row.id == id)
            .expect("bank");
        assert_eq!(bank.state, "Waiting");
        assert!(bank.starts_at.is_none());
        assert!(snap.orders.iter().all(|order| order.strategy != id));
    }

    #[test]
    fn seeded_put_is_frozen_at_the_fill_not_the_trigger() {
        let snap = seed().view();
        let nifty = snap
            .strategies
            .iter()
            .find(|row| row.id == "nifty-22500-strangle")
            .expect("seeded book");
        assert_eq!(nifty.state, "Live");
        assert_eq!(nifty.combined, 121_225);
        let put = nifty
            .legs
            .iter()
            .find(|leg| leg.symbol == "22500 PE")
            .expect("put");
        assert_eq!(put.open, 0);
        assert_eq!(put.exit, Some(13_385));
        assert_eq!(put.realized, -38_025);
        let call = nifty
            .legs
            .iter()
            .find(|leg| leg.symbol == "22500 CE")
            .expect("call");
        assert_eq!(call.open, 65);
        assert_eq!(call.unrealized, 159_250);
        let bank = snap
            .strategies
            .iter()
            .find(|row| row.id == "banknifty-48000-straddle")
            .expect("inactive book");
        assert_eq!(bank.state, "Waiting");
    }

    #[test]
    fn local_offset_is_a_timezone_not_a_placeholder() {
        let offset = local_offset_ms();
        assert!((-14 * 3_600_000..=14 * 3_600_000).contains(&offset));
        assert_eq!(offset % 60_000, 0);
    }
}
