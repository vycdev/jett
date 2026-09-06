include!("solution.rs");

fn stock(sku: &str, quantity: i64) -> InventoryEvent {
    InventoryEvent::Stock {
        sku: sku.to_string(),
        quantity,
    }
}

fn sale(sku: &str, quantity: i64) -> InventoryEvent {
    InventoryEvent::Sale {
        sku: sku.to_string(),
        quantity,
    }
}

fn balances(entries: &[(&str, i64)]) -> std::collections::BTreeMap<String, i64> {
    entries
        .iter()
        .map(|(sku, quantity)| ((*sku).to_string(), *quantity))
        .collect()
}

#[test]
fn hidden_inventory_batch() {
    assert_eq!(
        apply_inventory(vec![]),
        InventoryResult::Accepted(balances(&[]))
    );
    assert_eq!(
        apply_inventory(vec![
            stock("apple", 5),
            sale("apple", 2),
            stock("banana", 4)
        ]),
        InventoryResult::Accepted(balances(&[("apple", 3), ("banana", 4)]))
    );
    assert_eq!(
        apply_inventory(vec![stock("apple", 2), sale("apple", 2)]),
        InventoryResult::Accepted(balances(&[("apple", 0)]))
    );
    assert_eq!(
        apply_inventory(vec![sale("apple", 1)]),
        InventoryResult::Rejected {
            index: 0,
            sku: "apple".to_string()
        }
    );
    assert_eq!(
        apply_inventory(vec![
            stock("apple", 2),
            sale("apple", 3),
            stock("banana", 9)
        ]),
        InventoryResult::Rejected {
            index: 1,
            sku: "apple".to_string()
        }
    );
    assert_eq!(
        apply_inventory(vec![stock("bad", 0)]),
        InventoryResult::Rejected {
            index: 0,
            sku: "bad".to_string()
        }
    );
}
