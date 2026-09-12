function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function maxProductPair(values: readonly bigint[]): bigint | null {
    if ((BigInt(values.length) < 2n)) {
        return null;
    }
    let best: bigint = (nth(values, 0n) * nth(values, 1n));
    let first: bigint = 0n;
    while ((first < BigInt(values.length))) {
        let second: bigint = (first + 1n);
        while ((second < BigInt(values.length))) {
            let candidate: bigint = (nth(values, first) * nth(values, second));
            if ((candidate > best)) {
                best = candidate;
            }
            second = (second + 1n);
        }
        first = (first + 1n);
    }
    return best;
}
