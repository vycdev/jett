import { boundedWeightedSum } from "./solution.js";

function check(actual: bigint, expected: bigint): void {
  if (actual !== expected) throw new Error(`expected ${expected}, got ${actual}`);
}

check(boundedWeightedSum([], 5n), 0n);
check(boundedWeightedSum([1n, 2n, 3n], 10n), 14n);
check(boundedWeightedSum([20n, -20n, 3n], 10n), -1n);
check(boundedWeightedSum([-1n, 2n, -3n, 4n], 2n), 5n);
check(boundedWeightedSum([9n, -4n], 0n), 0n);
