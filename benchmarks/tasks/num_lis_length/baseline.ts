function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function lisLength(values: readonly bigint[]): bigint {
    let lengths: bigint[] = [];
    let best: bigint = 0n;
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        let ending: bigint = 1n;
        let prior: bigint = 0n;
        while ((prior < index)) {
            if ((nth(values, prior) < nth(values, index))) {
                let candidate: bigint = (nth(lengths, prior) + 1n);
                if ((candidate > ending)) {
                    ending = candidate;
                }
            }
            prior = (prior + 1n);
        }
        lengths.push(ending);
        if ((ending > best)) {
            best = ending;
        }
        index = (index + 1n);
    }
    return best;
}
