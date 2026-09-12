function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function equilibriumIndex(values: readonly bigint[]): bigint | null {
    let remaining: bigint = 0n;
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        remaining = (remaining + nth(values, index));
        index = (index + 1n);
    }
    let before: bigint = 0n;
    index = 0n;
    while ((index < BigInt(values.length))) {
        let value: bigint = nth(values, index);
        remaining = (remaining - value);
        if ((before == remaining)) {
            return index;
        }
        before = (before + value);
        index = (index + 1n);
    }
    return null;
}
