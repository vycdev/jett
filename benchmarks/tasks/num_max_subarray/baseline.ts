function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function maxSubarray(values: readonly bigint[]): bigint | null {
    if ((BigInt(values.length) == 0n)) {
        return null;
    }
    let ending: bigint = nth(values, 0n);
    let best: bigint = ending;
    let index: bigint = 1n;
    while ((index < BigInt(values.length))) {
        let value: bigint = nth(values, index);
        if ((ending > 0n)) {
            ending = (ending + value);
        } else {
            ending = value;
        }
        if ((ending > best)) {
            best = ending;
        }
        index = (index + 1n);
    }
    return best;
}
