function safeDiv(left: bigint, right: bigint): bigint { return left / right; }
function safeMod(left: bigint, right: bigint): bigint { return left % right; }
export function totient(n: bigint): bigint {
    let remaining: bigint = n;
    let count: bigint = n;
    let factor: bigint = 2n;
    while (((factor * factor) <= remaining)) {
        if ((safeMod(remaining, factor) == 0n)) {
            count = (count - safeDiv(count, factor));
            while ((safeMod(remaining, factor) == 0n)) {
                remaining = safeDiv(remaining, factor);
            }
        }
        factor = (factor + 1n);
    }
    if ((remaining > 1n)) {
        count = (count - safeDiv(count, remaining));
    }
    return count;
}
