function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function prefixBalances(values: readonly bigint[], opening: bigint): bigint[] {
    let balances: bigint[] = [opening];
    let balance: bigint = opening;
    let index: bigint = 0n;
    while ((index < BigInt(values.length))) {
        balance = (balance + nth(values, index));
        balances.push(balance);
        index = (index + 1n);
    }
    return balances;
}
