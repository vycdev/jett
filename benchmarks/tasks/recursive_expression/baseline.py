from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Literal:
    value: int


@dataclass(frozen=True)
class Add:
    left: Expr
    right: Expr


@dataclass(frozen=True)
class Divide:
    numerator: Expr
    denominator: Expr


@dataclass(frozen=True)
class Negate:
    inner: Expr


type Expr = Literal | Add | Divide | Negate


@dataclass(frozen=True)
class ValueResult:
    value: int


@dataclass(frozen=True)
class DivisionByZero:
    pass


type EvalResult = ValueResult | DivisionByZero


def divide_toward_zero(numerator: int, denominator: int) -> int:
    quotient = abs(numerator) // abs(denominator)
    if (numerator < 0) != (denominator < 0):
        return -quotient
    return quotient


def evaluate(expression: Expr) -> EvalResult:
    match expression:
        case Literal(value):
            return ValueResult(value)
        case Add(left, right):
            left_result = evaluate(left)
            if isinstance(left_result, DivisionByZero):
                return left_result
            right_result = evaluate(right)
            if isinstance(right_result, DivisionByZero):
                return right_result
            return ValueResult(left_result.value + right_result.value)
        case Divide(numerator, denominator):
            numerator_result = evaluate(numerator)
            if isinstance(numerator_result, DivisionByZero):
                return numerator_result
            denominator_result = evaluate(denominator)
            if isinstance(denominator_result, DivisionByZero):
                return denominator_result
            if denominator_result.value == 0:
                return DivisionByZero()
            return ValueResult(
                divide_toward_zero(numerator_result.value, denominator_result.value)
            )
        case Negate(inner):
            result = evaluate(inner)
            if isinstance(result, DivisionByZero):
                return result
            return ValueResult(-result.value)
