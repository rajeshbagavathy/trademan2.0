use std::collections::HashSet;

use broker_api::{ClientOrderId, Execution, OrderEvent, PlaceIntent, PositionView};
use tradecore::PriceTicks;

/// Scripted broker. Tests push events and assert the calls the engine made.
#[derive(Debug, Default)]
pub struct Sim {
    pub places: Vec<PlaceIntent>,
    pub modifies: Vec<(String, i64)>,
    pub cancels: Vec<String>,
    known: HashSet<String>,
    pending: Vec<OrderEvent>,
}

impl Sim {
    pub fn push(&mut self, event: OrderEvent) {
        self.pending.push(event);
    }
}

impl Execution for Sim {
    fn place(&mut self, intent: &PlaceIntent) {
        self.known.insert(intent.client_id.0.clone());
        self.places.push(intent.clone());
    }

    fn modify(&mut self, client_id: &ClientOrderId, trigger: PriceTicks) {
        self.modifies.push((client_id.0.clone(), trigger.get()));
    }

    fn cancel(&mut self, client_id: &ClientOrderId) {
        self.cancels.push(client_id.0.clone());
    }

    fn poll(&mut self) -> Vec<OrderEvent> {
        std::mem::take(&mut self.pending)
    }

    fn positions(&self) -> Vec<PositionView> {
        Vec::new()
    }

    fn knows(&self, client_id: &ClientOrderId) -> bool {
        self.known.contains(&client_id.0)
    }
}
