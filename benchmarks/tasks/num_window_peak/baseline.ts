function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function windowPeak(values: readonly bigint[], width: bigint): bigint | null {
    if ((width == 0n)) {
        return null;
    }
    if ((width > BigInt(values.length))) {
        return null;
    }
    let total: bigint = 0n;
    let index: bigint = 0n;
    while ((index < width)) {
        total = (total + nth(values, index));
        index = (index + 1n);
    }
    let best: bigint = total;
    while ((index < BigInt(values.length))) {
        total = ((total + nth(values, index)) - nth(values, (index - width)));
        if ((total > best)) {
            best = total;
        }
        index = (index + 1n);
    }
    return best;
}
