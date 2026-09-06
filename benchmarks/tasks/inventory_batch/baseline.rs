use std::collections::BTreeMap;

pub enum InventoryEvent {
    Stock { sku: String, quantity: i64 },
    Sale { sku: String, quantity: i64 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum InventoryResult {
    Accepted(BTreeMap<String, i64>),
    Rejected { index: i64, sku: String },
}

pub fn apply_inventory(events: Vec<InventoryEvent>) -> InventoryResult {
    let mut balances = BTreeMap::new();
    for (index, event) in events.into_iter().enumerate() {
        match event {
            InventoryEvent::Stock { sku, quantity } => {
                if quantity <= 0 {
                    return InventoryResult::Rejected {
                        index: index as i64,
                        sku,
                    };
                }
                *balances.entry(sku).or_insert(0) += quantity;
            }
            InventoryEvent::Sale { sku, quantity } => {
                let current = balances.get(&sku).copied().unwrap_or(0);
                if quantity <= 0 || quantity > current {
                    return InventoryResult::Rejected {
                        index: index as i64,
                        sku,
                    };
                }
                balances.insert(sku, current - quantity);
            }
        }
    }
    InventoryResult::Accepted(balances)
}
