function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function stablePartition(values: readonly bigint[]): bigint[] {
    let output: bigint[] = [];
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        let value: bigint = nth(values, index);
        if ((value < 0n)) {
            output.push(value);
        }
        index = (index + 1n);
    }
    index = 0n;
    while ((index < BigInt(values.length))) {
        let value: bigint = nth(values, index);
        if ((value >= 0n)) {
            output.push(value);
        }
        index = (index + 1n);
    }
    return output;
}
