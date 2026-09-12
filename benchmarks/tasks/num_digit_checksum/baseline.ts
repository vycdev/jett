function safeDiv(left: bigint, right: bigint): bigint { return left / right; }
function safeMod(left: bigint, right: bigint): bigint { return left % right; }
export function digitChecksum(n: bigint): bigint {
    let remaining: bigint = n;
    let sign: bigint = 1n;
    let checksum: bigint = 0n;
    while ((remaining > 0n)) {
        checksum = (checksum + (sign * safeMod(remaining, 10n)));
        sign = -(sign);
        remaining = safeDiv(remaining, 10n);
    }
    return checksum;
}
