function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function polynomialValue(coefficients: readonly bigint[], x: bigint): bigint {
    let answer: bigint = 0n;
    let index: bigint = BigInt(coefficients.length);
    while ((index > 0n)) {
        index = (index - 1n);
        answer = ((answer * x) + nth(coefficients, index));
    }
    return answer;
}
