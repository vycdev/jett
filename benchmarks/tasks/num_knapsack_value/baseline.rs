fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn knapsack_value(weights: &[i64], values: &[i64], capacity: i64) -> i64 {
    let mut costs: Vec<i64> = vec![];
    let mut size: i64 = 0;
    while (size <= capacity) {
        costs.push(0);
        size = (size + 1);
    }
    let mut item: i64 = 0;
    while (item < i64::try_from(weights.len()).unwrap_or(0)) {
        let mut next_costs: Vec<i64> = vec![];
        let mut room: i64 = 0;
        let mut weight: i64 = nth(&weights, item);
        let mut value: i64 = nth(&values, item);
        while (room <= capacity) {
            let mut best: i64 = nth(&costs, room);
            if (weight <= room) {
                let mut candidate: i64 = (nth(&costs, (room - weight)) + value);
                if (candidate > best) {
                    best = candidate;
                }
            }
            next_costs.push(best);
            room = (room + 1);
        }
        costs = next_costs;
        item = (item + 1);
    }
    return nth(&costs, capacity);
}
