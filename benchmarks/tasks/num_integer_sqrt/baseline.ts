function safeDiv(left: bigint, right: bigint): bigint { return left / right; }
export function integerSqrt(n: bigint): bigint {
    let lower: bigint = 0n;
    let upper: bigint = 1000001n;
    while (((lower + 1n) < upper)) {
        let middle: bigint = safeDiv((lower + upper), 2n);
        if (((middle * middle) <= n)) {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    return lower;
}
