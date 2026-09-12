export type Mode = { readonly value: bigint; readonly count: bigint };
function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
function occurrenceCount(values: readonly bigint[], target: bigint): bigint {
    let count: bigint = 0n;
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        if ((nth(values, index) == target)) {
            count = (count + 1n);
        }
        index = (index + 1n);
    }
    return count;
}
export function histogramMode(values: readonly bigint[]): Mode {
    let bestValue: bigint = 0n;
    let bestCount: bigint = 0n;
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        let value: bigint = nth(values, index);
        let count: bigint = occurrenceCount(values, value);
        let replace: boolean = (count > bestCount);
        if ((count == bestCount)) {
            if ((value < bestValue)) {
                replace = true;
            }
        }
        if (replace) {
            bestValue = value;
            bestCount = count;
        }
        index = (index + 1n);
    }
    return {value: bestValue, count: bestCount};
}
