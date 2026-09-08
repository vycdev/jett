include!("solution.rs");

fn boxed(expression: Expr) -> Box<Expr> {
    Box::new(expression)
}

#[test]
fn hidden_recursive_expression() {
    let tests = [
        (Expr::Literal(7), EvalResult::Value(7)),
        (
            Expr::Add(boxed(Expr::Literal(2)), boxed(Expr::Literal(3))),
            EvalResult::Value(5),
        ),
        (
            Expr::Negate(boxed(Expr::Add(
                boxed(Expr::Literal(4)),
                boxed(Expr::Literal(5)),
            ))),
            EvalResult::Value(-9),
        ),
        (
            Expr::Divide(boxed(Expr::Literal(8)), boxed(Expr::Literal(2))),
            EvalResult::Value(4),
        ),
        (
            Expr::Divide(boxed(Expr::Literal(8)), boxed(Expr::Literal(0))),
            EvalResult::DivisionByZero,
        ),
        (
            Expr::Divide(
                boxed(Expr::Literal(10)),
                boxed(Expr::Divide(
                    boxed(Expr::Literal(1)),
                    boxed(Expr::Literal(0)),
                )),
            ),
            EvalResult::DivisionByZero,
        ),
        (
            Expr::Add(
                boxed(Expr::Divide(
                    boxed(Expr::Literal(1)),
                    boxed(Expr::Literal(0)),
                )),
                boxed(Expr::Literal(5)),
            ),
            EvalResult::DivisionByZero,
        ),
        (
            Expr::Divide(
                boxed(Expr::Literal(21)),
                boxed(Expr::Negate(boxed(Expr::Literal(3)))),
            ),
            EvalResult::Value(-7),
        ),
        (
            Expr::Divide(boxed(Expr::Literal(-7)), boxed(Expr::Literal(2))),
            EvalResult::Value(-3),
        ),
        (
            Expr::Divide(boxed(Expr::Literal(7)), boxed(Expr::Literal(-2))),
            EvalResult::Value(-3),
        ),
    ];
    for (expression, expected) in tests {
        assert_eq!(evaluate(expression), expected);
    }
}
