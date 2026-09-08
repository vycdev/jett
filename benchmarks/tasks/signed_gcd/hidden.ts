import { signedGcd } from "./solution.js";

function check(actual: bigint, expected: bigint): void {
  if (actual !== expected) throw new Error(`expected ${expected}, got ${actual}`);
}

check(signedGcd(0n, 0n), 0n);
check(signedGcd(54n, 24n), 6n);
check(signedGcd(-54n, 24n), 6n);
check(signedGcd(54n, -24n), 6n);
check(signedGcd(-7n, -13n), 1n);
check(signedGcd(0n, 42n), 42n);
check(signedGcd(270n, 192n), 6n);
