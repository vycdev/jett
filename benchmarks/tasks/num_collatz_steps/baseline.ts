function safeDiv(left: bigint, right: bigint): bigint { return left / right; }
function safeMod(left: bigint, right: bigint): bigint { return left % right; }
export function collatzSteps(n: bigint, limit: bigint): bigint | null {
    let current: bigint = n;
    let steps: bigint = 0n;
    while ((current != 1n)) {
        if ((steps == limit)) {
            return null;
        }
        if ((safeMod(current, 2n) == 0n)) {
            current = safeDiv(current, 2n);
        } else {
            current = ((3n * current) + 1n);
        }
        steps = (steps + 1n);
    }
    return steps;
}
