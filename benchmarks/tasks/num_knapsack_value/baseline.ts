function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function knapsackValue(weights: readonly bigint[], values: readonly bigint[], capacity: bigint): bigint {
    let costs: bigint[] = [];
    let size: bigint = 0n;
    while ((size <= capacity)) {
        costs.push(0n);
        size = (size + 1n);
    }
    let item: bigint = 0n;
    while ((item < BigInt(weights.length))) {
        let nextCosts: bigint[] = [];
        let room: bigint = 0n;
        let weight: bigint = nth(weights, item);
        let value: bigint = nth(values, item);
        while ((room <= capacity)) {
            let best: bigint = nth(costs, room);
            if ((weight <= room)) {
                let candidate: bigint = (nth(costs, (room - weight)) + value);
                if ((candidate > best)) {
                    best = candidate;
                }
            }
            nextCosts.push(best);
            room = (room + 1n);
        }
        costs = nextCosts;
        item = (item + 1n);
    }
    return nth(costs, capacity);
}
