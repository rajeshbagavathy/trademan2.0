use std::collections::{HashSet, VecDeque};

use broker_api::{BrokerOrderId, ClientOrderId, Execution, OrderEvent, PlaceIntent, PositionView};
use tradecore::PriceTicks;

#[derive(Debug, Default)]
pub struct Paper {
    pub calls: Vec<String>,
    events: VecDeque<OrderEvent>,
    known: HashSet<String>,
    next_id: u64,
}

impl Paper {
    pub fn fill(&mut self, client_id: ClientOrderId, qty: tradecore::QtyUnits, price: PriceTicks) {
        self.events.push_back(OrderEvent::Fill {
            client_id,
            qty,
            price,
        });
    }

    pub fn reject(&mut self, client_id: ClientOrderId, reason: broker_api::RejectReason) {
        self.events
            .push_back(OrderEvent::Reject { client_id, reason });
    }
}

impl Execution for Paper {
    fn place(&mut self, intent: &PlaceIntent) {
        self.calls.push(format!("place:{}", intent.client_id.0));
        self.known.insert(intent.client_id.0.clone());
        self.next_id += 1;
        self.events.push_back(OrderEvent::Ack {
            client_id: intent.client_id.clone(),
            broker_id: BrokerOrderId(self.next_id.to_string()),
        });
    }

    fn modify(&mut self, client_id: &ClientOrderId, trigger: PriceTicks) {
        self.calls
            .push(format!("modify:{}:{}", client_id.0, trigger.get()));
        self.events.push_back(OrderEvent::Replaced {
            client_id: client_id.clone(),
            trigger,
        });
    }

    fn cancel(&mut self, client_id: &ClientOrderId) {
        self.calls.push(format!("cancel:{}", client_id.0));
        self.events.push_back(OrderEvent::Canceled {
            client_id: client_id.clone(),
        });
    }

    fn poll(&mut self) -> Vec<OrderEvent> {
        self.events.drain(..).collect()
    }

    fn positions(&self) -> Vec<PositionView> {
        Vec::new()
    }

    fn knows(&self, client_id: &ClientOrderId) -> bool {
        self.known.contains(&client_id.0)
    }
}
