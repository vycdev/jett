function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function inversionCount(values: readonly bigint[]): bigint {
    let count: bigint = 0n;
    let first: bigint = 0n;
    while ((first < BigInt(values.length))) {
        let second: bigint = (first + 1n);
        while ((second < BigInt(values.length))) {
            if ((nth(values, first) > nth(values, second))) {
                count = (count + 1n);
            }
            second = (second + 1n);
        }
        first = (first + 1n);
    }
    return count;
}
