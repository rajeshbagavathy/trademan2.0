/// Price in integer ticks. One tick is the smallest price step the engine stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PriceTicks(i64);

impl PriceTicks {
    pub const fn new(ticks: i64) -> Self {
        Self(ticks)
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    pub const fn saturating_sub(self, other: Self) -> i64 {
        self.0.saturating_sub(other.0)
    }
}

/// Quantity in exchange units. Lot-size conversion belongs in a broker adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QtyUnits(u64);

impl QtyUnits {
    pub const fn new(units: u64) -> Self {
        Self(units)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fill {
    pub price: PriceTicks,
    pub qty: QtyUnits,
}

/// Volume-weighted average price. A zero-quantity fill contributes nothing.
/// `None` when every fill has zero quantity.
pub fn vwap(fills: &[Fill]) -> Option<PriceTicks> {
    let mut notional: i128 = 0;
    let mut qty: i128 = 0;
    for fill in fills {
        let units = i128::from(fill.qty.get());
        if units == 0 {
            continue;
        }
        notional += i128::from(fill.price.get()) * units;
        qty += units;
    }
    if qty == 0 {
        None
    } else {
        Some(PriceTicks::new(i64::try_from(notional / qty).unwrap_or(0)))
    }
}

/// Signed points times units. Sell profit is entry minus exit.
pub fn signed_pnl(entry: PriceTicks, exit: PriceTicks, qty: QtyUnits, sell: bool) -> i64 {
    let points = if sell {
        entry.saturating_sub(exit)
    } else {
        exit.saturating_sub(entry)
    };
    points.saturating_mul(i64::try_from(qty.get()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vwap_weights_fills_and_ignores_zero_quantity() {
        let fills = [
            Fill {
                price: PriceTicks::new(100),
                qty: QtyUnits::new(65),
            },
            Fill {
                price: PriceTicks::new(200),
                qty: QtyUnits::new(65),
            },
            Fill {
                price: PriceTicks::new(999),
                qty: QtyUnits::new(0),
            },
        ];
        assert_eq!(vwap(&fills), Some(PriceTicks::new(150)));
    }

    #[test]
    fn vwap_is_none_when_every_slice_is_zero() {
        let fills = [Fill {
            price: PriceTicks::new(100),
            qty: QtyUnits::new(0),
        }];
        assert_eq!(vwap(&fills), None);
    }

    #[test]
    fn quantity_adds_in_units() {
        let one_lot = QtyUnits::new(65);
        let two_lots = one_lot.saturating_add(QtyUnits::new(65));
        assert_eq!(two_lots.get(), 130);
    }
}
