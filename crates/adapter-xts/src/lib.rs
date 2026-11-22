use std::collections::{HashMap, VecDeque};

use broker_api::{ClientOrderId, InstrumentId, Tick};
use serde_json::Value;
use tradecore::{PriceTicks, QtyUnits};

pub fn parse_market(frame: &Value) -> Option<Tick> {
    let code = frame.get("messageCode")?.as_i64()?;
    if code != 1501 {
        return None;
    }
    let price = frame.get("lastTradedPrice")?.as_f64()?;
    let exchange_ts_ms = frame.get("exchangeTimestamp")?.as_i64()?;
    let instrument = frame.get("exchangeInstrumentID")?.as_i64()?.to_string();
    Some(Tick {
        instrument: InstrumentId(instrument),
        exchange_ts_ms,
        price: PriceTicks::new((price * 100.0).round() as i64),
        feed: "xts".to_string(),
    })
}

pub fn units_to_lots(units: u64, lot_size: u64) -> u64 {
    units.checked_div(lot_size).unwrap_or(0)
}

pub fn lots_to_units(lots: u64, lot_size: u64) -> u64 {
    lots.saturating_mul(lot_size)
}

#[derive(Debug, Default)]
pub struct TagMap {
    next: u64,
    to_tag: HashMap<String, String>,
    to_core: HashMap<String, String>,
}

impl TagMap {
    pub fn tag(&mut self, core_id: &str) -> String {
        if let Some(tag) = self.to_tag.get(core_id) {
            return tag.clone();
        }
        self.next += 1;
        let tag = format!("t{:x}", self.next);
        debug_assert!(tag.len() <= 20);
        self.to_tag.insert(core_id.to_string(), tag.clone());
        self.to_core.insert(tag.clone(), core_id.to_string());
        tag
    }

    pub fn core_id(&self, tag: &str) -> Option<&str> {
        self.to_core.get(tag).map(String::as_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestingStop {
    Pending,
    Live(i64),
}

pub fn on_http_request_sent(current: Option<i64>) -> Option<i64> {
    current
}

pub fn on_replaced(trigger: PriceTicks) -> RestingStop {
    RestingStop::Live(trigger.get())
}

pub fn client_from_tag(map: &TagMap, tag: &str) -> Option<ClientOrderId> {
    map.core_id(tag).map(|id| ClientOrderId(id.to_string()))
}

pub struct OrderBucket {
    times: VecDeque<i64>,
}

impl OrderBucket {
    pub fn new() -> Self {
        Self {
            times: VecDeque::new(),
        }
    }

    pub fn try_send(&mut self, now_ms: i64) -> bool {
        while self
            .times
            .front()
            .is_some_and(|stamp| now_ms.saturating_sub(*stamp) >= 1_000)
        {
            self.times.pop_front();
        }
        if self.times.len() >= 10 {
            return false;
        }
        self.times.push_back(now_ms);
        true
    }
}

impl Default for OrderBucket {
    fn default() -> Self {
        Self::new()
    }
}

pub fn qty_units(lots_in_payload: u64, lot_size: u64) -> QtyUnits {
    QtyUnits::new(lots_to_units(lots_in_payload, lot_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touchline_becomes_ticks_and_other_frames_are_ignored() {
        let touch: Value =
            serde_json::from_str(include_str!("../../../fixtures/xts/touchline.json")).unwrap();
        let tick = parse_market(&touch).unwrap();
        assert_eq!(tick.price.get(), 14250);
        assert_eq!(tick.exchange_ts_ms, 1710000000000);
        let depth: Value =
            serde_json::from_str(include_str!("../../../fixtures/xts/depth.json")).unwrap();
        let candle: Value =
            serde_json::from_str(include_str!("../../../fixtures/xts/candle.json")).unwrap();
        assert!(parse_market(&depth).is_none());
        assert!(parse_market(&candle).is_none());
    }

    #[test]
    fn order_bucket_delays_the_eleventh_call() {
        let mut bucket = OrderBucket::new();
        let mut queued = 0;
        for _ in 0..11 {
            if !bucket.try_send(0) {
                queued += 1;
            }
        }
        assert_eq!(queued, 1);
        assert!(bucket.try_send(1_000));
    }

    #[test]
    fn http_accept_does_not_move_the_resting_trigger() {
        let resting = Some(100);
        assert_eq!(on_http_request_sent(resting), Some(100));
        assert_eq!(on_replaced(PriceTicks::new(90)), RestingStop::Live(90));
    }

    #[test]
    fn broker_tag_round_trips_and_stays_short() {
        let mut map = TagMap::default();
        let core_id = "strategy-very-long-identifier-that-exceeds-twenty";
        let tag = map.tag(core_id);
        assert!(tag.len() <= 20);
        assert_eq!(map.core_id(&tag), Some(core_id));
        assert_eq!(client_from_tag(&map, &tag).unwrap().0, core_id);
    }

    #[test]
    fn lots_convert_both_ways() {
        assert_eq!(units_to_lots(130, 65), 2);
        assert_eq!(lots_to_units(2, 65), 130);
        assert_eq!(qty_units(2, 65).get(), 130);
    }
}
