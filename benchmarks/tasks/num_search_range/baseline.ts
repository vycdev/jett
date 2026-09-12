export type SearchRange = { readonly first: bigint; readonly last: bigint };
function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function searchRange(values: readonly bigint[], target: bigint): SearchRange {
    let first: bigint = -(1n);
    let last: bigint = -(1n);
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        if ((nth(values, index) == target)) {
            if ((first == -(1n))) {
                first = index;
            }
            last = index;
        }
        index = (index + 1n);
    }
    return {first: first, last: last};
}
