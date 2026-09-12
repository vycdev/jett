function safeDiv(left: bigint, right: bigint): bigint { return left / right; }
export function binomial(n: bigint, k: bigint): bigint {
    if ((k > n)) {
        return 0n;
    }
    let answer: bigint = 1n;
    let index: bigint = 1n;
    while ((index <= k)) {
        answer = safeDiv((answer * ((n - index) + 1n)), index);
        index = (index + 1n);
    }
    return answer;
}
