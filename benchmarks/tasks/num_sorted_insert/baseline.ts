function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function sortedInsert(values: readonly bigint[], value: bigint): bigint[] {
    let output: bigint[] = [];
    let inserted: boolean = false;
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        let item: bigint = nth(values, index);
        if (!(inserted)) {
            if ((value <= item)) {
                output.push(value);
                inserted = true;
            }
        }
        output.push(item);
        index = (index + 1n);
    }
    if (!(inserted)) {
        output.push(value);
    }
    return output;
}
