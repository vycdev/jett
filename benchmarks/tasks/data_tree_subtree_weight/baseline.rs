#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub node: i64,
    pub parent: i64,
    pub weight: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub node_count: i64,
    pub total_weight: i64,
    pub leaf_count: i64,
}

pub fn belongs(rows: &[Entry], node: i64, selected: i64) -> bool {
    let mut current: i64 = node;
    while (current != 0) {
        if (current == selected) {
            return true;
        }
        let mut parent: i64 = 0;
        for row in rows {
            if (row.node == current) {
                parent = row.parent;
            }
        }
        current = parent;
    }
    return false;
}

pub fn is_leaf(rows: &[Entry], node: i64) -> bool {
    for row in rows {
        if (row.parent == node) {
            return false;
        }
    }
    return true;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut count: i64 = 0;
    let mut total: i64 = 0;
    let mut leaves: i64 = 0;
    for row in rows {
        if belongs(rows, row.node, limit) {
            count = (count + 1);
            total = (total + row.weight);
            if is_leaf(rows, row.node) {
                leaves = (leaves + 1);
            }
        }
    }
    return Report {
        node_count: count,
        total_weight: total,
        leaf_count: leaves,
    };
}
