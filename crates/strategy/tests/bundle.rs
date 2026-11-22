use broker_api::{ClientOrderId, InstrumentId, Intent, LegId, OrderEvent, OrderRole, Side};
use strategy::{Book, LegSpec, Momentum, Monitor, Params, Phase, SlMode, TrailMode};
use tradecore::{PriceTicks, QtyUnits};

fn leg(id: u32, symbol: &str, qty: u64) -> LegSpec {
    LegSpec {
        id: LegId(id),
        instrument: InstrumentId(symbol.to_string()),
        side: Side::Sell,
        target_qty: QtyUnits::new(qty),
        risk: strategy::LegRisk::default(),
    }
}

fn place_id(intent: &Intent) -> String {
    match intent {
        Intent::Place(place) => place.client_id.0.clone(),
        _ => String::new(),
    }
}

fn ack(id: &str) -> OrderEvent {
    OrderEvent::Ack {
        client_id: ClientOrderId(id.to_string()),
        broker_id: broker_api::BrokerOrderId(format!("b-{id}")),
    }
}

fn fill(id: &str, qty: u64, price: i64) -> OrderEvent {
    OrderEvent::Fill {
        client_id: ClientOrderId(id.to_string()),
        qty: QtyUnits::new(qty),
        price: PriceTicks::new(price),
    }
}

fn live_two() -> (Book, String, String) {
    let params = Params {
        fill_window_ms: 1_000,
        strategy_stop: Some(-100),
        ..Params::default()
    };
    let mut book = Book::new("s", params, vec![leg(1, "CE", 1), leg(2, "PE", 1)]);
    let entries = book.activate(0);
    let id1 = place_id(&entries[0]);
    let id2 = place_id(&entries[1]);
    book.on_event(&ack(&id1));
    book.on_event(&fill(&id1, 1, 100));
    book.on_event(&ack(&id2));
    let stops = book.on_event(&fill(&id2, 1, 100));
    let stop1 = place_id(&stops[0]);
    let stop2 = place_id(&stops[1]);
    book.on_event(&ack(&stop1));
    book.on_event(&ack(&stop2));
    assert_eq!(book.phase(), Phase::Live);
    (book, stop1, stop2)
}

#[test]
fn product_defaults_to_mis() {
    assert_eq!(Params::default().product, broker_api::Product::Mis);
}

#[test]
fn margin_reject_on_second_leg_flattens_the_first() {
    let mut book = Book::new(
        "s",
        Params::default(),
        vec![leg(1, "CE", 1), leg(2, "PE", 1)],
    );
    let entries = book.activate(0);
    let id1 = place_id(&entries[0]);
    let id2 = place_id(&entries[1]);
    book.on_event(&ack(&id1));
    book.on_event(&fill(&id1, 1, 100));
    let flatten = book.on_event(&OrderEvent::Reject {
        client_id: ClientOrderId(id2),
        reason: broker_api::RejectReason::Margin,
    });
    assert_eq!(book.phase(), Phase::Aborting);
    assert!(flatten
        .iter()
        .any(|intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Exit)));
    let exit = flatten
        .iter()
        .find_map(|intent| match intent {
            Intent::Place(place) if place.role == OrderRole::Exit => {
                Some(place.client_id.0.clone())
            }
            _ => None,
        })
        .unwrap();
    book.on_event(&fill(&exit, 1, 110));
    assert_eq!(book.phase(), Phase::Flat);
    assert!(book.legs().iter().all(|leg| leg.open == 0));
}

#[test]
fn partial_second_leg_is_cancelled_and_both_are_exited() {
    let params = Params {
        fill_window_ms: 100,
        ..Params::default()
    };
    let mut book = Book::new("s", params, vec![leg(1, "CE", 2), leg(2, "PE", 2)]);
    let entries = book.activate(0);
    let id1 = place_id(&entries[0]);
    let id2 = place_id(&entries[1]);
    book.on_event(&ack(&id1));
    book.on_event(&fill(&id1, 2, 100));
    book.on_event(&ack(&id2));
    book.on_event(&fill(&id2, 1, 100));
    let flatten = book.on_time(100);
    assert_eq!(book.phase(), Phase::Aborting);
    assert!(flatten
        .iter()
        .any(|intent| matches!(intent, Intent::Cancel { client_id } if client_id.0 == id2)));
    let exits: Vec<_> = flatten
        .iter()
        .filter_map(|intent| match intent {
            Intent::Place(place) if place.role == OrderRole::Exit => {
                Some(place.client_id.0.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(exits.len(), 2);
    for exit in exits {
        let qty = book
            .legs()
            .iter()
            .find(|leg| leg.open > 0)
            .map(|leg| leg.open)
            .unwrap_or(0);
        book.on_event(&fill(&exit, qty.max(1), 110));
    }
    assert_eq!(book.phase(), Phase::Flat);
}

#[test]
fn ack_timeout_exits_filled_legs() {
    let params = Params {
        fill_window_ms: 50,
        ..Params::default()
    };
    let mut book = Book::new(
        "s",
        params,
        vec![leg(1, "CE", 1), leg(2, "PE", 1), leg(3, "CE2", 1)],
    );
    let entries = book.activate(0);
    let filled: Vec<_> = entries.iter().take(2).map(place_id).collect();
    let missing = place_id(&entries[2]);
    for id in &filled {
        book.on_event(&ack(id));
        book.on_event(&fill(id, 1, 100));
    }
    let flatten = book.on_time(50);
    assert!(flatten
        .iter()
        .any(|intent| matches!(intent, Intent::Cancel { client_id } if client_id.0 == missing)));
    assert_eq!(
        flatten
            .iter()
            .filter(
                |intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Exit)
            )
            .count(),
        2
    );
}

#[test]
fn rejected_stop_never_goes_live() {
    let mut book = Book::new("s", Params::default(), vec![leg(1, "CE", 1)]);
    let entries = book.activate(0);
    let id = place_id(&entries[0]);
    book.on_event(&ack(&id));
    let stops = book.on_event(&fill(&id, 1, 100));
    let stop = place_id(&stops[0]);
    book.on_event(&OrderEvent::Reject {
        client_id: ClientOrderId(stop),
        reason: broker_api::RejectReason::Unknown,
    });
    assert_ne!(book.phase(), Phase::Live);
    assert_eq!(book.phase(), Phase::Aborting);
}

#[test]
fn full_fills_and_stop_acks_go_live() {
    let (book, _, _) = live_two();
    assert_eq!(book.phase(), Phase::Live);
}

#[test]
fn restart_while_aborting_only_flattens() {
    let (book, intents) = Book::resume_aborting("s", Params::default(), vec![leg(1, "CE", 1)]);
    assert_eq!(book.phase(), Phase::Aborting);
    assert!(intents
        .iter()
        .all(|intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Exit)));
}

#[test]
fn duplicate_fill_does_not_increase_quantity() {
    let mut book = Book::new("s", Params::default(), vec![leg(1, "CE", 1)]);
    let entries = book.activate(0);
    let id = place_id(&entries[0]);
    book.on_event(&ack(&id));
    book.on_event(&fill(&id, 1, 100));
    book.on_event(&fill(&id, 1, 100));
    assert_eq!(book.legs()[0].open, 1);
}

#[test]
fn closed_put_stays_frozen_when_its_tick_moves() {
    let (mut book, _, stop_put) = live_two();
    book.on_event(&fill(&stop_put, 1, 110));
    let frozen = book.combined_pnl();
    book.on_tick(&InstrumentId("PE".to_string()), PriceTicks::new(300), 1);
    assert_eq!(book.combined_pnl(), frozen);
    book.on_tick(&InstrumentId("CE".to_string()), PriceTicks::new(80), 2);
    assert_ne!(book.combined_pnl(), frozen);
    let put = book
        .legs()
        .into_iter()
        .find(|leg| leg.id == LegId(2))
        .unwrap();
    assert_eq!(put.exit, Some(110));
    assert_eq!(put.trigger, Some(100));
}

#[test]
fn exit_price_is_the_traded_average_not_the_trigger() {
    let mut book = Book::new("s", Params::default(), vec![leg(1, "CE", 3)]);
    let entries = book.activate(0);
    let id = place_id(&entries[0]);
    book.on_event(&ack(&id));
    book.on_event(&fill(&id, 3, 100));
    assert_eq!(book.legs()[0].open, 3);
    let stop_events = {
        let mut replay = Book::new("s", Params::default(), vec![leg(1, "CE", 3)]);
        let entries = replay.activate(0);
        let id = place_id(&entries[0]);
        replay.on_event(&ack(&id));
        replay.on_event(&fill(&id, 1, 10));
        replay.on_event(&fill(&id, 2, 20));
        replay
    };
    assert_eq!(stop_events.legs()[0].entry, Some(16));
}

#[test]
fn candle_close_ignores_a_wick_and_ltp_does_not() {
    let params = Params {
        strategy_stop: Some(-100),
        monitor: Monitor::Candle { ms: 60_000 },
        ..Params::default()
    };
    let mut book = Book::new("s", params, vec![leg(1, "CE", 1)]);
    let entries = book.activate(0);
    let id = place_id(&entries[0]);
    book.on_event(&ack(&id));
    let stops = book.on_event(&fill(&id, 1, 100));
    book.on_event(&ack(&place_id(&stops[0])));
    assert!(book
        .on_tick(&InstrumentId("CE".to_string()), PriceTicks::new(250), 0)
        .is_empty());
    book.on_tick(&InstrumentId("CE".to_string()), PriceTicks::new(120), 1_000);
    assert!(book.on_time(60_000).is_empty());
    assert_eq!(book.phase(), Phase::Live);

    let params = Params {
        strategy_stop: Some(-100),
        monitor: Monitor::Ltp,
        ..Params::default()
    };
    let mut fast = Book::new("s", params, vec![leg(1, "CE", 1)]);
    let entries = fast.activate(0);
    let id = place_id(&entries[0]);
    fast.on_event(&ack(&id));
    let stops = fast.on_event(&fill(&id, 1, 100));
    fast.on_event(&ack(&place_id(&stops[0])));
    let intents = fast.on_tick(&InstrumentId("CE".to_string()), PriceTicks::new(250), 0);
    assert!(intents
        .iter()
        .any(|intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Exit)));
}

#[test]
fn candle_close_beyond_the_stop_exits() {
    let params = Params {
        strategy_stop: Some(-100),
        monitor: Monitor::Candle { ms: 60_000 },
        ..Params::default()
    };
    let mut book = Book::new("s", params, vec![leg(1, "CE", 1)]);
    let entries = book.activate(0);
    let id = place_id(&entries[0]);
    book.on_event(&ack(&id));
    let stops = book.on_event(&fill(&id, 1, 100));
    book.on_event(&ack(&place_id(&stops[0])));
    book.on_tick(&InstrumentId("CE".to_string()), PriceTicks::new(250), 0);
    let intents = book.on_time(60_000);
    assert!(intents
        .iter()
        .any(|intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Exit)));
}

#[test]
fn stale_open_leg_blocks_the_strategy_stop() {
    let (mut book, _, _) = live_two();
    book.set_stale(&InstrumentId("PE".to_string()));
    let intents = book.on_tick(&InstrumentId("CE".to_string()), PriceTicks::new(400), 10);
    assert!(intents.is_empty());
    assert_eq!(book.phase(), Phase::Live);
}

#[test]
fn square_off_without_ticks_exits_open_quantity() {
    let (mut book, _, _) = live_two();
    let intents = book.square_off();
    assert_eq!(book.phase(), Phase::Exiting);
    assert_eq!(
        intents
            .iter()
            .filter(
                |intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Exit)
            )
            .count(),
        2
    );
}

#[test]
fn unset_wait_and_trail_do_not_block_entry() {
    let params = Params {
        wait_distance: None,
        trail_distance: None,
        ..Params::default()
    };
    let mut book = Book::new("s", params, vec![leg(1, "CE", 1)]);
    assert_eq!(book.activate(0).len(), 1);
}

fn live_one(spec: LegSpec, params: Params) -> Book {
    let qty = spec.target_qty.get();
    let mut book = Book::new("s", params, vec![spec]);
    let entries = book.activate(0);
    let id = place_id(&entries[0]);
    book.on_event(&ack(&id));
    let stops = book.on_event(&fill(&id, qty, 100));
    book.on_event(&ack(&place_id(&stops[0])));
    book
}

#[test]
fn sell_stop_is_entry_plus_the_leg_points() {
    let mut spec = leg(1, "CE", 1);
    spec.risk.sl = SlMode::Points;
    spec.risk.sl_value = 25;
    let book = live_one(spec, Params::default());
    assert_eq!(book.legs()[0].trigger, Some(125));
}

#[test]
fn sell_percent_stop_uses_the_entry_premium() {
    let mut spec = leg(1, "CE", 1);
    spec.risk.sl = SlMode::Percent;
    spec.risk.sl_value = 40;
    let mut book = Book::new("s", Params::default(), vec![spec]);
    let entries = book.activate(0);
    let id = place_id(&entries[0]);
    book.on_event(&ack(&id));
    let stops = book.on_event(&fill(&id, 1, 200));
    match &stops[0] {
        Intent::Place(place) => assert_eq!(place.trigger.map(|price| price.get()), Some(280)),
        other => panic!("expected a stop, got {other:?}"),
    }
}

#[test]
fn leg_trail_steps_the_stop_from_its_origin() {
    let mut spec = leg(1, "CE", 1);
    spec.risk.sl = SlMode::Points;
    spec.risk.sl_value = 25;
    spec.risk.trail = TrailMode::Points;
    spec.risk.trail_every = 10;
    spec.risk.trail_by = 5;
    let params = Params {
        monitor: Monitor::Ltp,
        ..Params::default()
    };
    let mut book = live_one(spec, params);
    let intents = book.on_tick(&InstrumentId("CE".into()), PriceTicks::new(80), 1);
    match &intents[0] {
        Intent::Modify { trigger, .. } => assert_eq!(trigger.get(), 115),
        other => panic!("expected a trail modify, got {other:?}"),
    }
}

#[test]
fn overall_target_exits_on_ltp() {
    let params = Params {
        strategy_target: Some(40),
        monitor: Monitor::Ltp,
        ..Params::default()
    };
    let mut book = live_one(leg(1, "CE", 1), params);
    let intents = book.on_tick(&InstrumentId("CE".into()), PriceTicks::new(60), 1);
    assert!(intents
        .iter()
        .any(|intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Exit)));
}

#[test]
fn overall_premium_percent_holds_every_entry_until_the_combined_premium_moves() {
    let params = Params {
        momentum: Momentum::PremiumUp(10),
        ..Params::default()
    };
    let mut book = Book::new("s", params, vec![leg(1, "CE", 1), leg(2, "PE", 1)]);
    assert!(book.activate(0).is_empty());
    assert!(book
        .on_tick(&InstrumentId("CE".into()), PriceTicks::new(100), 1)
        .is_empty());
    assert!(book
        .on_tick(&InstrumentId("PE".into()), PriceTicks::new(100), 2)
        .is_empty());
    let fired = book.on_tick(&InstrumentId("CE".into()), PriceTicks::new(120), 3);
    let symbols: Vec<_> = fired
        .iter()
        .filter_map(|intent| match intent {
            Intent::Place(place) if place.role == OrderRole::Entry => {
                Some(place.instrument.0.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(symbols, vec!["CE", "PE"]);
}

#[test]
fn premium_percent_momentum_holds_the_entry_until_the_option_moves() {
    let mut spec = leg(1, "CE", 1);
    spec.risk.momentum = Momentum::PremiumUp(10);
    let mut book = Book::new("s", Params::default(), vec![spec]);
    assert!(book.activate(0).is_empty());
    assert_eq!(book.phase(), Phase::Waiting);
    assert!(book
        .on_tick(&InstrumentId("CE".into()), PriceTicks::new(10_000), 1)
        .is_empty());
    assert!(book
        .on_tick(&InstrumentId("CE".into()), PriceTicks::new(10_999), 2)
        .is_empty());
    let fired = book.on_tick(&InstrumentId("CE".into()), PriceTicks::new(11_000), 3);
    match &fired[0] {
        Intent::Place(place) => {
            assert_eq!(place.role, OrderRole::Entry);
            assert_eq!(place.instrument.0, "CE");
        }
        other => panic!("expected the entry, got {other:?}"),
    }
}

#[test]
fn overall_trail_lifts_the_stop_before_the_exit() {
    let params = Params {
        strategy_stop: Some(-100),
        overall_trail_every: Some(50),
        overall_trail_by: Some(20),
        monitor: Monitor::Ltp,
        ..Params::default()
    };
    let mut book = live_one(leg(1, "CE", 1), params);
    assert!(book
        .on_tick(&InstrumentId("CE".into()), PriceTicks::new(50), 1)
        .is_empty());
    let intents = book.on_tick(&InstrumentId("CE".into()), PriceTicks::new(190), 2);
    assert!(intents
        .iter()
        .any(|intent| matches!(intent, Intent::Place(place) if place.role == OrderRole::Exit)));
}
