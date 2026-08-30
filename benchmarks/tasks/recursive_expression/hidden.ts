import { evaluate, type EvalResult, type Expr } from "./solution.js";

function check(expression: Expr, expected: EvalResult): void {
  const actual = evaluate(expression);
  if (actual.kind !== expected.kind) throw new Error("unexpected result kind");
  if (actual.kind === "value" && expected.kind === "value" && actual.value !== expected.value) {
    throw new Error("unexpected result value");
  }
}

const literal = (value: bigint): Expr => ({ kind: "literal", value });
check(literal(7n), { kind: "value", value: 7n });
check({ kind: "add", left: literal(2n), right: literal(3n) }, { kind: "value", value: 5n });
check({ kind: "negate", inner: { kind: "add", left: literal(4n), right: literal(5n) } }, { kind: "value", value: -9n });
check({ kind: "divide", numerator: literal(8n), denominator: literal(2n) }, { kind: "value", value: 4n });
check({ kind: "divide", numerator: literal(8n), denominator: literal(0n) }, { kind: "division_by_zero" });
check({ kind: "divide", numerator: literal(10n), denominator: { kind: "divide", numerator: literal(1n), denominator: literal(0n) } }, { kind: "division_by_zero" });
check({ kind: "add", left: { kind: "divide", numerator: literal(1n), denominator: literal(0n) }, right: literal(5n) }, { kind: "division_by_zero" });
check({ kind: "divide", numerator: literal(21n), denominator: { kind: "negate", inner: literal(3n) } }, { kind: "value", value: -7n });
check({ kind: "divide", numerator: literal(-7n), denominator: literal(2n) }, { kind: "value", value: -3n });
check({ kind: "divide", numerator: literal(7n), denominator: literal(-2n) }, { kind: "value", value: -3n });
