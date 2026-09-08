export type Expr =
  | { readonly kind: "literal"; readonly value: bigint }
  | { readonly kind: "add"; readonly left: Expr; readonly right: Expr }
  | { readonly kind: "divide"; readonly numerator: Expr; readonly denominator: Expr }
  | { readonly kind: "negate"; readonly inner: Expr };

export type EvalResult =
  | { readonly kind: "value"; readonly value: bigint }
  | { readonly kind: "division_by_zero" };

function negateResult(result: EvalResult): EvalResult {
  switch (result.kind) {
    case "value":
      return { kind: "value", value: -result.value };
    case "division_by_zero":
      return result;
  }
}

function addResults(left: EvalResult, right: EvalResult): EvalResult {
  switch (left.kind) {
    case "value":
      switch (right.kind) {
        case "value":
          return { kind: "value", value: left.value + right.value };
        case "division_by_zero":
          return right;
      }
    case "division_by_zero":
      return left;
  }
}

function divideResults(numerator: EvalResult, denominator: EvalResult): EvalResult {
  switch (numerator.kind) {
    case "value":
      switch (denominator.kind) {
        case "value":
          if (denominator.value === 0n) return { kind: "division_by_zero" };
          return { kind: "value", value: numerator.value / denominator.value };
        case "division_by_zero":
          return denominator;
      }
    case "division_by_zero":
      return numerator;
  }
}

export function evaluate(expression: Expr): EvalResult {
  switch (expression.kind) {
    case "literal":
      return { kind: "value", value: expression.value };
    case "add":
      return addResults(evaluate(expression.left), evaluate(expression.right));
    case "divide":
      return divideResults(evaluate(expression.numerator), evaluate(expression.denominator));
    case "negate":
      return negateResult(evaluate(expression.inner));
  }
}
