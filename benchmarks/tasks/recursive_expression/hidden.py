from solution import Add, Divide, DivisionByZero, Expr, Literal, Negate, ValueResult, evaluate


def check(expression: Expr, expected: ValueResult | DivisionByZero) -> None:
    assert evaluate(expression) == expected


check(Literal(7), ValueResult(7))
check(Add(Literal(2), Literal(3)), ValueResult(5))
check(Negate(Add(Literal(4), Literal(5))), ValueResult(-9))
check(Divide(Literal(8), Literal(2)), ValueResult(4))
check(Divide(Literal(8), Literal(0)), DivisionByZero())
check(Divide(Literal(10), Divide(Literal(1), Literal(0))), DivisionByZero())
check(Add(Divide(Literal(1), Literal(0)), Literal(5)), DivisionByZero())
check(Divide(Literal(21), Negate(Literal(3))), ValueResult(-7))
check(Divide(Literal(-7), Literal(2)), ValueResult(-3))
check(Divide(Literal(7), Literal(-2)), ValueResult(-3))
