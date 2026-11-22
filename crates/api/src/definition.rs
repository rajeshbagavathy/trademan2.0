use broker_api::{InstrumentId, LegId, Product, Side};
use serde::{Deserialize, Serialize};
use strategy::{Book, LegRisk, LegSpec, Monitor, Params, SlMode, TrailMode};
use tradecore::QtyUnits;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    pub name: String,
    pub underlying: String,
    pub underlying_from: String,
    pub strategy_type: String,
    pub entry_time: String,
    pub exit_time: String,
    pub square_off: String,
    pub breakeven: String,
    pub legs: Vec<LegDraft>,
    pub overall: Overall,
    pub execution: Execution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegDraft {
    pub position: String,
    pub option_type: String,
    pub expiry: String,
    pub strike: String,
    pub strike_value: f64,
    pub lots: u64,
    pub sl: String,
    pub sl_value: f64,
    pub trail: String,
    pub trail_every: f64,
    pub trail_by: f64,
    pub target: String,
    pub target_value: f64,
    pub momentum: String,
    pub momentum_value: f64,
    pub range_breakout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Overall {
    pub target: String,
    pub target_value: f64,
    pub stop: String,
    pub stop_value: f64,
    pub trailing: String,
    pub lock_at: f64,
    pub lock_profit: f64,
    pub trail_every: f64,
    pub trail_by: f64,
    pub momentum: String,
    pub momentum_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub multiplier: u32,
    pub monitor: String,
    pub product: String,
    pub entry_order: String,
    pub exit_order: String,
    pub buffer_type: String,
    pub trigger_buffer: f64,
    pub limit_buffer: f64,
    pub ref_price: String,
    pub leg_monitoring: String,
    pub frequency_seconds: u32,
    pub auto_square_margin: bool,
    pub delay_seconds: u32,
    pub days: Vec<String>,
    pub dte: Vec<u32>,
}

impl Definition {
    pub fn sample(name: &str, underlying: &str, strike: f64, lots: u64) -> Self {
        let leg = |option_type: &str| LegDraft {
            position: "sell".into(),
            option_type: option_type.into(),
            expiry: "weekly".into(),
            strike: "atm".into(),
            strike_value: strike,
            lots,
            sl: "none".into(),
            sl_value: 0.0,
            trail: "none".into(),
            trail_every: 0.0,
            trail_by: 0.0,
            target: "none".into(),
            target_value: 0.0,
            momentum: "none".into(),
            momentum_value: 0.0,
            range_breakout: "none".into(),
        };
        Self {
            name: name.into(),
            underlying: underlying.into(),
            underlying_from: "cash".into(),
            strategy_type: "intraday".into(),
            entry_time: "09:20".into(),
            exit_time: "15:20".into(),
            square_off: "partial".into(),
            breakeven: "none".into(),
            legs: vec![leg("CE"), leg("PE")],
            overall: Overall {
                target: "none".into(),
                target_value: 0.0,
                stop: "mtm".into(),
                stop_value: if underlying == "BANKNIFTY" {
                    3000.0
                } else {
                    2000.0
                },
                trailing: "none".into(),
                lock_at: 0.0,
                lock_profit: 0.0,
                trail_every: 0.0,
                trail_by: 0.0,
                momentum: "none".into(),
                momentum_value: 0.0,
            },
            execution: Execution {
                multiplier: 1,
                monitor: "candle-1".into(),
                product: "MIS".into(),
                entry_order: "limit".into(),
                exit_order: "limit".into(),
                buffer_type: "points".into(),
                trigger_buffer: 0.0,
                limit_buffer: 0.0,
                ref_price: "traded".into(),
                leg_monitoring: "continuous".into(),
                frequency_seconds: 1,
                auto_square_margin: true,
                delay_seconds: 0,
                days: vec![
                    "mon".into(),
                    "tue".into(),
                    "wed".into(),
                    "thu".into(),
                    "fri".into(),
                ],
                dte: vec![0, 1, 2, 3, 4],
            },
        }
    }
}

pub fn materialize(id: &str, definition: &Definition) -> Book {
    let params = params_from(definition);
    let specs = definition
        .legs
        .iter()
        .enumerate()
        .map(|(index, leg)| LegSpec {
            id: LegId(index as u32 + 1),
            instrument: InstrumentId(symbol(definition, leg)),
            side: if leg.position.eq_ignore_ascii_case("buy") {
                Side::Buy
            } else {
                Side::Sell
            },
            target_qty: QtyUnits::new(quantity(definition, leg)),
            risk: risk_of(leg),
        })
        .collect();
    Book::new(id, params, specs)
}

fn quantity(definition: &Definition, leg: &LegDraft) -> u64 {
    let lots = leg.lots.max(1);
    let multiplier = u64::from(definition.execution.multiplier.max(1));
    lots.saturating_mul(lot_size(&definition.underlying))
        .saturating_mul(multiplier)
}

fn lot_size(underlying: &str) -> u64 {
    match underlying {
        "BANKNIFTY" => 30,
        "SENSEX" => 20,
        "MIDCPNIFTY" => 50,
        _ => 65,
    }
}

fn symbol(definition: &Definition, leg: &LegDraft) -> String {
    let strike = resolve_strike(&definition.underlying, &leg.option_type, &leg.strike);
    format!("{} {strike} {}", definition.underlying, leg.option_type)
}

/// Paper spot used when a saved ATM, ITM, or OTM choice becomes an order.
/// ITM and OTM are the nearest strike, one step from ATM.
fn resolve_strike(underlying: &str, option_type: &str, selection: &str) -> i64 {
    let step = match underlying {
        "BANKNIFTY" | "SENSEX" => 100,
        "MIDCPNIFTY" => 25,
        _ => 50,
    };
    let spot = match underlying {
        "BANKNIFTY" => 48_000,
        "FINNIFTY" => 23_500,
        "MIDCPNIFTY" => 12_000,
        "SENSEX" => 80_000,
        _ => 22_500,
    };
    let atm = (spot + step / 2) / step * step;
    let shift = match (selection, option_type) {
        ("itm", "PE") | ("otm", "CE") => step,
        ("itm", "CE") | ("otm", "PE") => -step,
        _ => 0,
    };
    atm + shift
}

fn params_from(definition: &Definition) -> Params {
    let money = |value: f64| (value * 100.0).round() as i64;
    let overall = &definition.overall;
    let mut params = Params {
        monitor: match definition.execution.monitor.as_str() {
            "ltp" => Monitor::Ltp,
            "candle-3" => Monitor::Candle { ms: 180_000 },
            _ => Monitor::Candle { ms: 60_000 },
        },
        product: if definition.execution.product == "NRML" {
            Product::Nrml
        } else {
            Product::Mis
        },
        ..Params::default()
    };
    match overall.stop.as_str() {
        "mtm" => params.strategy_stop = Some(-money(overall.stop_value).abs()),
        "premium" => params.stop_premium_pct = Some(overall.stop_value.round() as i64),
        _ => {}
    }
    match overall.target.as_str() {
        "mtm" => params.strategy_target = Some(money(overall.target_value).abs()),
        "premium" => params.target_premium_pct = Some(overall.target_value.round() as i64),
        _ => {}
    }
    match overall.trailing.as_str() {
        "lock" => {
            params.lock_arm = Some(money(overall.lock_at));
            params.lock_floor = Some(money(overall.lock_profit));
        }
        "lock_trail" => {
            params.lock_arm = Some(money(overall.lock_at));
            params.lock_floor = Some(money(overall.lock_profit));
            params.lock_trails = true;
            params.overall_trail_every = Some(money(overall.trail_every));
            params.overall_trail_by = Some(money(overall.trail_by));
        }
        "overall_trail" => {
            params.overall_trail_every = Some(money(overall.trail_every));
            params.overall_trail_by = Some(money(overall.trail_by));
        }
        _ => {}
    }
    params.momentum = book_momentum(overall);
    params
}

fn book_momentum(overall: &Overall) -> strategy::Momentum {
    let percent = overall.momentum_value.round() as i64;
    let points = (overall.momentum_value * 100.0).round() as i64;
    match overall.momentum.as_str() {
        "percent_up" if percent > 0 => strategy::Momentum::PremiumUp(percent),
        "percent_down" if percent > 0 => strategy::Momentum::PremiumDown(percent),
        "points_up" if points > 0 => strategy::Momentum::PointsUp(points),
        "points_down" if points > 0 => strategy::Momentum::PointsDown(points),
        _ => strategy::Momentum::Off,
    }
}

fn risk_of(leg: &LegDraft) -> LegRisk {
    let points = |value: f64| (value * 100.0).round() as i64;
    let sl = match leg.sl.as_str() {
        "points" => SlMode::Points,
        "percent" => SlMode::Percent,
        "underlying_points" => SlMode::UnderlyingPoints,
        "underlying_percent" => SlMode::UnderlyingPercent,
        _ => SlMode::Off,
    };
    let trail = if leg.trail == "points" {
        TrailMode::Points
    } else {
        TrailMode::Off
    };
    let sl_value = if matches!(sl, SlMode::Percent | SlMode::UnderlyingPercent) {
        leg.sl_value.round() as i64
    } else {
        points(leg.sl_value)
    };
    let trail_every = points(leg.trail_every);
    let trail_by = points(leg.trail_by);
    LegRisk {
        sl,
        sl_value,
        trail,
        trail_every,
        trail_by,
        momentum: momentum_of(leg),
    }
}

fn momentum_of(leg: &LegDraft) -> strategy::Momentum {
    let percent = leg.momentum_value.round() as i64;
    if percent <= 0 {
        return strategy::Momentum::Off;
    }
    match leg.momentum.as_str() {
        "percent_up" => strategy::Momentum::PremiumUp(percent),
        "percent_down" => strategy::Momentum::PremiumDown(percent),
        _ => strategy::Momentum::Off,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use broker_api::{Intent, OrderRole};
    use tradecore::PriceTicks;

    fn orders_of(definition: &Definition) -> Vec<String> {
        let mut book = materialize("paper", definition);
        book.activate(0)
            .into_iter()
            .filter_map(|intent| match intent {
                Intent::Place(place) if place.role == OrderRole::Entry => Some(place.instrument.0),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn strike_choice_is_the_instrument_on_the_entry_order() {
        let mut definition = Definition::sample("paper", "NIFTY", 0.0, 1);
        definition.legs[0].strike = "otm".into();
        definition.legs[0].option_type = "CE".into();
        definition.legs[1].strike = "itm".into();
        definition.legs[1].option_type = "PE".into();
        assert_eq!(
            orders_of(&definition),
            vec!["NIFTY 22550 CE".to_string(), "NIFTY 22550 PE".to_string()]
        );
    }

    #[test]
    fn premium_percent_on_the_leg_holds_that_entry_order() {
        let mut definition = Definition::sample("paper", "NIFTY", 0.0, 1);
        definition.legs[0].strike = "atm".into();
        definition.legs[0].momentum = "percent_up".into();
        definition.legs[0].momentum_value = 10.0;
        definition.legs[1].strike = "atm".into();
        let mut book = materialize("paper", &definition);
        let first = book.activate(0);
        let symbols: Vec<_> = first
            .iter()
            .filter_map(|intent| match intent {
                Intent::Place(place) if place.role == OrderRole::Entry => {
                    Some(place.instrument.0.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(symbols, vec!["NIFTY 22500 PE"]);
        assert!(book
            .on_tick(
                &InstrumentId("NIFTY 22500 CE".into()),
                PriceTicks::new(15_000),
                1
            )
            .is_empty());
        let fired = book.on_tick(
            &InstrumentId("NIFTY 22500 CE".into()),
            PriceTicks::new(16_500),
            2,
        );
        assert!(fired.iter().any(|intent| matches!(
            intent,
            Intent::Place(place) if place.role == OrderRole::Entry && place.instrument.0 == "NIFTY 22500 CE"
        )));
    }

    #[test]
    fn point_trail_modifies_the_stop_that_was_created_with_the_entry() {
        let mut definition = Definition::sample("paper", "NIFTY", 0.0, 1);
        definition.legs.truncate(1);
        definition.legs[0].strike = "atm".into();
        definition.legs[0].option_type = "CE".into();
        definition.legs[0].position = "sell".into();
        definition.legs[0].sl = "points".into();
        definition.legs[0].sl_value = 25.0;
        definition.legs[0].trail = "points".into();
        definition.legs[0].trail_every = 10.0;
        definition.legs[0].trail_by = 5.0;
        definition.execution.monitor = "ltp".into();
        let mut book = materialize("paper", &definition);
        let entries = book.activate(0);
        let Intent::Place(entry) = &entries[0] else {
            panic!("entry missing");
        };
        let entry_id = entry.client_id.clone();
        book.on_event(&broker_api::OrderEvent::Ack {
            client_id: entry_id.clone(),
            broker_id: broker_api::BrokerOrderId("e".into()),
        });
        let stops = book.on_event(&broker_api::OrderEvent::Fill {
            client_id: entry_id,
            qty: tradecore::QtyUnits::new(65),
            price: PriceTicks::new(10_000),
        });
        let Intent::Place(stop) = &stops[0] else {
            panic!("stop missing");
        };
        assert_eq!(stop.instrument.0, "NIFTY 22500 CE");
        assert_eq!(stop.trigger.map(|price| price.get()), Some(12_500));
        book.on_event(&broker_api::OrderEvent::Ack {
            client_id: stop.client_id.clone(),
            broker_id: broker_api::BrokerOrderId("s".into()),
        });
        let moved = book.on_tick(
            &InstrumentId("NIFTY 22500 CE".into()),
            PriceTicks::new(9_000),
            1,
        );
        assert!(moved.iter().any(|intent| matches!(
            intent,
            Intent::Modify { trigger, .. } if trigger.get() == 12_000
        )));
    }

    #[test]
    fn overall_percent_momentum_holds_the_entry_orders() {
        let mut definition = Definition::sample("paper", "NIFTY", 0.0, 1);
        definition.overall.momentum = "percent_up".into();
        definition.overall.momentum_value = 10.0;
        let mut book = materialize("paper", &definition);
        assert!(book.activate(0).is_empty());
    }
}
