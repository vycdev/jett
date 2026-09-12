function safeDiv(left: bigint, right: bigint): bigint { return left / right; }
function safeMod(left: bigint, right: bigint): bigint { return left % right; }
export function divisorSum(n: bigint): bigint {
    let total: bigint = 0n;
    let divisor: bigint = 1n;
    while (((divisor * divisor) <= n)) {
        if ((safeMod(n, divisor) == 0n)) {
            total = (total + divisor);
            let partner: bigint = safeDiv(n, divisor);
            if ((partner != divisor)) {
                total = (total + partner);
            }
        }
        divisor = (divisor + 1n);
    }
    return total;
}
