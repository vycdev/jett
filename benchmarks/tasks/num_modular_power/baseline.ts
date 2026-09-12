function safeDiv(left: bigint, right: bigint): bigint { return left / right; }
function safeMod(left: bigint, right: bigint): bigint { return left % right; }
export function modularPower(base: bigint, exponent: bigint, modulus: bigint): bigint {
    let power: bigint = safeMod(base, modulus);
    let remaining: bigint = exponent;
    let answer: bigint = safeMod(1n, modulus);
    while ((remaining > 0n)) {
        if ((safeMod(remaining, 2n) == 1n)) {
            answer = safeMod((answer * power), modulus);
        }
        power = safeMod((power * power), modulus);
        remaining = safeDiv(remaining, 2n);
    }
    return answer;
}
