function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
function safeMod(left: bigint, right: bigint): bigint { return left % right; }
export function rotateLeft(values: readonly bigint[], distance: bigint): bigint[] {
    let rotated: bigint[] = [];
    let count: bigint = BigInt(values.length);
    if ((count == 0n)) {
        return rotated;
    }
    let shift: bigint = safeMod(distance, count);
    let index: bigint = 0n;
    while ((index < count)) {
        let position: bigint = safeMod((index + shift), count);
        rotated.push(nth(values, position));
        index = (index + 1n);
    }
    return rotated;
}
