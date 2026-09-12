#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub node: i64,
    pub parent: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub ancestor: i64,
    pub selected_distance: i64,
    pub largest_distance: i64,
}

pub fn parent_of(rows: &[Entry], node: i64) -> i64 {
    for row in rows {
        if (row.node == node) {
            return row.parent;
        }
    }
    return (-1);
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut largest: i64 = 0;
    for row in rows {
        if (row.node > largest) {
            largest = row.node;
        }
    }
    if (parent_of(rows, limit) == (-1)) {
        return Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1),
        };
    }
    let mut selected: i64 = limit;
    let mut selected_distance: i64 = 0;
    while (selected > 0) {
        let mut other: i64 = largest;
        let mut other_distance: i64 = 0;
        while (other > 0) {
            if (other == selected) {
                return Report {
                    ancestor: selected,
                    selected_distance: selected_distance,
                    largest_distance: other_distance,
                };
            }
            other = parent_of(rows, other);
            other_distance = (other_distance + 1);
        }
        selected = parent_of(rows, selected);
        selected_distance = (selected_distance + 1);
    }
    return Report {
        ancestor: 0,
        selected_distance: (-1),
        largest_distance: (-1),
    };
}
