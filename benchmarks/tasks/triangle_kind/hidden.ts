import { triangleKind } from "./solution.js";

function check(actual: string, expected: string): void {
  if (actual !== expected) throw new Error(`expected ${expected}, got ${actual}`);
}

check(triangleKind(3, 3, 3), "equilateral");
check(triangleKind(5, 5, 8), "isosceles");
check(triangleKind(3, 4, 5), "scalene");
check(triangleKind(1, 2, 3), "invalid");
check(triangleKind(0, 4, 4), "invalid");
check(triangleKind(-1, 2, 2), "invalid");
check(triangleKind(10, 3, 3), "invalid");
