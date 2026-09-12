function safeMod(left: bigint, right: bigint): bigint { return left % right; }
export function primeStatus(n: bigint): boolean {
    if ((n < 2n)) {
        return false;
    }
    let divisor: bigint = 2n;
    while (((divisor * divisor) <= n)) {
        if ((safeMod(n, divisor) == 0n)) {
            return false;
        }
        divisor = (divisor + 1n);
    }
    return true;
}
