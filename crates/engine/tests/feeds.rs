use adapter_paper::Paper;
use adapter_sim::Sim;
use broker_api::{ClientOrderId, InstrumentId, Intent, IntentPriority, Tick};
use engine::{Arbiter, OrderManager, TickGate};
use tradecore::PriceTicks;

fn tick(feed: &str, symbol: &str, ts: i64, price: i64) -> Tick {
    Tick {
        instrument: InstrumentId(symbol.to_string()),
        exchange_ts_ms: ts,
        price: PriceTicks::new(price),
        feed: feed.to_string(),
    }
}

#[test]
fn quieter_feed_goes_stale_and_fresher_tick_is_forwarded() {
    let mut arbiter = Arbiter::new(1_000);
    let first = tick("firstock", "NIFTY", 100, 10);
    assert!(arbiter.on_tick(&first, 0).is_some());
    let _ = arbiter.on_tick(&tick("xts", "NIFTY", 50, 9), 0);
    let fresh = tick("xts", "NIFTY", 200, 11);
    let forwarded = arbiter.on_tick(&fresh, 1_500).unwrap();
    assert_eq!(forwarded.feed, "xts");
    let stale = arbiter.stale_feeds(1_500);
    assert!(stale.iter().any(|feed| feed == "firstock"));
    assert!(!stale.iter().any(|feed| feed == "xts"));
}

#[test]
fn out_of_order_and_jump_ticks_are_dropped() {
    let mut gate = TickGate::new(20);
    assert!(gate.accept(&tick("a", "NIFTY", 100, 100)));
    assert!(!gate.accept(&tick("a", "NIFTY", 90, 101)));
    assert!(!gate.accept(&tick("a", "NIFTY", 110, 200)));
    assert!(gate.accept(&tick("a", "NIFTY", 120, 110)));
}

#[test]
fn disabling_one_feed_leaves_the_other() {
    let mut arbiter = Arbiter::new(5_000);
    arbiter.set_enabled("firstock", true);
    arbiter.set_enabled("xts", true);
    arbiter.set_enabled("firstock", false);
    assert!(arbiter
        .on_tick(&tick("firstock", "NIFTY", 100, 10), 0)
        .is_none());
    assert!(arbiter.on_tick(&tick("xts", "NIFTY", 100, 10), 0).is_some());
}

#[test]
fn in_flight_modifies_collapse_to_the_latest_trigger() {
    let mut manager = OrderManager::new(Sim::default());
    let id = ClientOrderId("stop-1".to_string());
    for trigger in [10, 11, 12] {
        manager.submit(Intent::Modify {
            client_id: id.clone(),
            trigger: PriceTicks::new(trigger),
            priority: IntentPriority::Trail,
        });
    }
    assert_eq!(manager.exec.modifies, vec![("stop-1".to_string(), 10)]);
    manager.on_replaced(&id, PriceTicks::new(10));
    assert_eq!(
        manager.exec.modifies,
        vec![("stop-1".to_string(), 10), ("stop-1".to_string(), 12)]
    );
}

#[test]
fn journal_is_written_before_place_and_timeout_does_not_place_again() {
    let mut manager = OrderManager::new(Paper::default());
    let intent = Intent::Place(broker_api::PlaceIntent {
        client_id: ClientOrderId("order-1".to_string()),
        strategy_id: broker_api::StrategyId("s".to_string()),
        leg_id: broker_api::LegId(1),
        instrument: InstrumentId("NIFTY".to_string()),
        side: broker_api::Side::Sell,
        qty: tradecore::QtyUnits::new(65),
        role: broker_api::OrderRole::Entry,
        trigger: None,
    });
    manager.submit(intent.clone());
    assert_eq!(
        manager.journal.ids.first().map(String::as_str),
        Some("order-1")
    );
    assert_eq!(manager.exec.calls.len(), 1);
    assert!(manager.on_timeout(&ClientOrderId("order-1".to_string())));
    manager.submit(intent);
    assert_eq!(manager.exec.calls.len(), 1);
}
