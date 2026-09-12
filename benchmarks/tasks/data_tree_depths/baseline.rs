#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub node: i64,
    pub parent: i64,
    pub weight: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub roots: i64,
    pub max_depth: i64,
    pub weighted_depth: i64,
}

pub fn depth_of(rows: &[Entry], node: i64) -> i64 {
    let mut current: i64 = node;
    let mut depth: i64 = (-1);
    while (current != 0) {
        let mut parent: i64 = 0;
        for row in rows {
            if (row.node == current) {
                parent = row.parent;
            }
        }
        current = parent;
        depth = (depth + 1);
    }
    return depth;
}

pub fn solve(rows: &[Entry]) -> Report {
    let mut roots: i64 = 0;
    let mut maximum: i64 = 0;
    let mut total: i64 = 0;
    for row in rows {
        let mut depth: i64 = depth_of(rows, row.node);
        if (depth == 0) {
            roots = (roots + 1);
        }
        if (depth > maximum) {
            maximum = depth;
        }
        total = (total + (row.weight * depth));
    }
    return Report {
        roots: roots,
        max_depth: maximum,
        weighted_depth: total,
    };
}
