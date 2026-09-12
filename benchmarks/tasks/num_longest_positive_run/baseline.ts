function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function longestPositiveRun(values: readonly bigint[]): bigint {
    let current: bigint = 0n;
    let best: bigint = 0n;
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        if ((nth(values, index) > 0n)) {
            current = (current + 1n);
            if ((current > best)) {
                best = current;
            }
        } else {
            current = 0n;
        }
        index = (index + 1n);
    }
    return best;
}
