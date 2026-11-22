use broker_api::{BrokerOrderId, ClientOrderId, InstrumentId, OrderEvent, Tick};
use serde_json::Value;
use tradecore::{PriceTicks, QtyUnits};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedTick {
    pub tick: Tick,
}

pub fn parse_market(frame: &Value) -> Option<Tick> {
    let price_raw = frame.get("i_last_traded_price")?.as_i64()?;
    let factor = frame
        .get("prcftr")
        .and_then(|value| value.as_str())
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|factor| *factor > 0.0)
        .unwrap_or(1.0);
    let ticks = (price_raw as f64 / factor).round() as i64;
    let exchange_ts_ms = frame.get("i_feed_time")?.as_i64()?;
    let symbol = frame
        .get("c_symbol")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    Some(Tick {
        instrument: InstrumentId(symbol.to_string()),
        exchange_ts_ms,
        price: PriceTicks::new(ticks),
        feed: "firstock".to_string(),
    })
}

pub fn parse_order(frame: &Value) -> Option<OrderEvent> {
    let broker_id = frame.get("norenordno")?.as_str()?.to_string();
    let remarks = frame
        .get("remarks")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let client_id = ClientOrderId(remarks.to_string());
    let status = frame
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let report = frame
        .get("reporttype")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if status == "COMPLETE" || report.eq_ignore_ascii_case("Fill") {
        let price = parse_decimal_ticks(frame.get("prc")?.as_str()?);
        let qty = frame
            .get("qty")
            .and_then(|value| value.as_str())
            .unwrap_or("0");
        let qty = qty.parse::<u64>().unwrap_or(0);
        return Some(OrderEvent::Fill {
            client_id,
            qty: QtyUnits::new(qty),
            price: PriceTicks::new(price),
        });
    }
    if status == "REJECTED" {
        return Some(OrderEvent::Reject {
            client_id,
            reason: broker_api::RejectReason::Unknown,
        });
    }
    Some(OrderEvent::Ack {
        client_id,
        broker_id: BrokerOrderId(broker_id),
    })
}

pub fn encode_sl_mkt_modify(order_number: &str, trigger: PriceTicks) -> Value {
    serde_json::json!({
        "orderNumber": order_number,
        "priceType": "SL-MKT",
        "triggerPrice": format!("{:.2}", trigger.get() as f64 / 100.0),
        "price": "0"
    })
}

pub fn connection_alive(last_pong_ms: Option<i64>, now_ms: i64) -> bool {
    last_pong_ms.is_some_and(|pong| now_ms.saturating_sub(pong) < 10_000)
}

fn parse_decimal_ticks(text: &str) -> i64 {
    let value: f64 = text.parse().unwrap_or(0.0);
    (value * 100.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_frame_uses_price_factor_and_exchange_time() {
        let frame: Value =
            serde_json::from_str(include_str!("../../../fixtures/firstock/market.json")).unwrap();
        let tick = parse_market(&frame).unwrap();
        assert_eq!(tick.price.get(), 14250);
        assert_eq!(tick.exchange_ts_ms, 1710000000000);
        assert_eq!(tick.instrument.0, "NSE:12345");
    }

    #[test]
    fn order_frame_maps_broker_id_remarks_and_traded_price() {
        let frame: Value =
            serde_json::from_str(include_str!("../../../fixtures/firstock/order.json")).unwrap();
        let event = parse_order(&frame).unwrap();
        match event {
            OrderEvent::Fill {
                client_id,
                qty,
                price,
            } => {
                assert_eq!(client_id.0, "s1-1-stop-1");
                assert_eq!(qty.get(), 65);
                assert_eq!(price.get(), 13385);
            }
            other => panic!("expected fill, got {other:?}"),
        }
    }

    #[test]
    fn sl_mkt_modify_keeps_the_order_number() {
        let body = encode_sl_mkt_modify("25042100011120", PriceTicks::new(11800));
        assert_eq!(body["orderNumber"], "25042100011120");
        assert_eq!(body["priceType"], "SL-MKT");
        assert_eq!(body["triggerPrice"], "118.00");
    }

    #[test]
    fn missing_pong_kills_the_session() {
        assert!(!connection_alive(None, 10_000));
        assert!(connection_alive(Some(0), 9_999));
        assert!(!connection_alive(Some(0), 10_000));
    }
}
