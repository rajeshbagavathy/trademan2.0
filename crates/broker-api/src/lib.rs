use tradecore::{PriceTicks, QtyUnits};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstrumentId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientOrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BrokerOrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StrategyId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LegId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub const fn is_sell(self) -> bool {
        matches!(self, Self::Sell)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Product {
    #[default]
    Mis,
    Nrml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    Margin,
    Risk,
    PriceBand,
    Freeze,
    RateLimit,
    Duplicate,
    Timeout,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderRole {
    Entry,
    Stop,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tick {
    pub instrument: InstrumentId,
    pub exchange_ts_ms: i64,
    pub price: PriceTicks,
    pub feed: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceIntent {
    pub client_id: ClientOrderId,
    pub strategy_id: StrategyId,
    pub leg_id: LegId,
    pub instrument: InstrumentId,
    pub side: Side,
    pub qty: QtyUnits,
    pub role: OrderRole,
    pub trigger: Option<PriceTicks>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Place(PlaceIntent),
    Modify {
        client_id: ClientOrderId,
        trigger: PriceTicks,
        priority: IntentPriority,
    },
    Cancel {
        client_id: ClientOrderId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntentPriority {
    Trail = 0,
    Stop = 1,
    Flatten = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderEvent {
    Ack {
        client_id: ClientOrderId,
        broker_id: BrokerOrderId,
    },
    Reject {
        client_id: ClientOrderId,
        reason: RejectReason,
    },
    Fill {
        client_id: ClientOrderId,
        qty: QtyUnits,
        price: PriceTicks,
    },
    Replaced {
        client_id: ClientOrderId,
        trigger: PriceTicks,
    },
    Canceled {
        client_id: ClientOrderId,
    },
}

impl OrderEvent {
    pub fn client_id(&self) -> &ClientOrderId {
        match self {
            Self::Ack { client_id, .. }
            | Self::Reject { client_id, .. }
            | Self::Fill { client_id, .. }
            | Self::Replaced { client_id, .. }
            | Self::Canceled { client_id } => client_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionView {
    pub instrument: InstrumentId,
    pub qty: QtyUnits,
    pub side: Side,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOrderView {
    pub client_id: ClientOrderId,
    pub broker_id: BrokerOrderId,
    pub trigger: Option<PriceTicks>,
}

pub trait Execution {
    fn place(&mut self, intent: &PlaceIntent);
    fn modify(&mut self, client_id: &ClientOrderId, trigger: PriceTicks);
    fn cancel(&mut self, client_id: &ClientOrderId);
    fn poll(&mut self) -> Vec<OrderEvent>;
    fn positions(&self) -> Vec<PositionView>;
    fn knows(&self, client_id: &ClientOrderId) -> bool;
}
