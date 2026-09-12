function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function uniqueSorted(values: readonly bigint[]): bigint[] {
    let output: bigint[] = [];
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        let value: bigint = nth(values, index);
        if ((index == 0n)) {
            output.push(value);
        } else {
            if ((value != nth(values, (index - 1n)))) {
                output.push(value);
            }
        }
        index = (index + 1n);
    }
    return output;
}
