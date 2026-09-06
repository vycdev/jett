export function boundedWeightedSum(values: readonly bigint[], cap: bigint): bigint {
  let total = 0n;
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    const bounded = value > cap ? cap : value < -cap ? -cap : value;
    total += bounded * BigInt(index + 1);
  }
  return total;
}
