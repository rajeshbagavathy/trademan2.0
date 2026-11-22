use adapter_paper::Paper;
use broker_api::Execution;

#[test]
fn paper_records_place_modify_cancel_and_emits_ack_and_fill() {
    let mut paper = Paper::default();
    let intent = broker_api::PlaceIntent {
        client_id: broker_api::ClientOrderId("c1".to_string()),
        strategy_id: broker_api::StrategyId("s".to_string()),
        leg_id: broker_api::LegId(1),
        instrument: broker_api::InstrumentId("NIFTY".to_string()),
        side: broker_api::Side::Sell,
        qty: tradecore::QtyUnits::new(65),
        role: broker_api::OrderRole::Entry,
        trigger: None,
    };
    paper.place(&intent);
    paper.modify(&intent.client_id, tradecore::PriceTicks::new(10));
    paper.cancel(&intent.client_id);
    paper.fill(
        intent.client_id.clone(),
        tradecore::QtyUnits::new(65),
        tradecore::PriceTicks::new(100),
    );
    let events = paper.poll();
    assert!(paper.calls.iter().any(|call| call.starts_with("place:")));
    assert!(paper.calls.iter().any(|call| call.starts_with("modify:")));
    assert!(paper.calls.iter().any(|call| call.starts_with("cancel:")));
    assert!(events
        .iter()
        .any(|event| matches!(event, broker_api::OrderEvent::Ack { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, broker_api::OrderEvent::Fill { .. })));
}
