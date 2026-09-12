function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function matrixTrace(values: readonly bigint[], rows: bigint): bigint {
    let total: bigint = 0n;
    let row: bigint = 0n;
    while ((row < rows)) {
        total = (total + nth(values, ((row * rows) + row)));
        row = (row + 1n);
    }
    return total;
}
