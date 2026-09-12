function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function coinChange(coins: readonly bigint[], amount: bigint): bigint {
    let costs: bigint[] = [0n];
    let total: bigint = 1n;
    while ((total <= amount)) {
        let best: bigint = 1000n;
        let index: bigint = 0n;
        while ((index < BigInt(coins.length))) {
            let coin: bigint = nth(coins, index);
            if ((coin <= total)) {
                let prior: bigint = nth(costs, (total - coin));
                if (((prior + 1n) < best)) {
                    best = (prior + 1n);
                }
            }
            index = (index + 1n);
        }
        costs.push(best);
        total = (total + 1n);
    }
    let answer: bigint = nth(costs, amount);
    if ((answer == 1000n)) {
        return -(1n);
    }
    return answer;
}
