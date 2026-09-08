pub enum Expr {
    Literal(i64),
    Add(Box<Expr>, Box<Expr>),
    Divide(Box<Expr>, Box<Expr>),
    Negate(Box<Expr>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum EvalResult {
    Value(i64),
    DivisionByZero,
}

fn negate_result(result: EvalResult) -> EvalResult {
    match result {
        EvalResult::Value(value) => EvalResult::Value(-value),
        EvalResult::DivisionByZero => EvalResult::DivisionByZero,
    }
}

fn add_results(left: EvalResult, right: EvalResult) -> EvalResult {
    match left {
        EvalResult::Value(left_value) => match right {
            EvalResult::Value(right_value) => EvalResult::Value(left_value + right_value),
            EvalResult::DivisionByZero => EvalResult::DivisionByZero,
        },
        EvalResult::DivisionByZero => EvalResult::DivisionByZero,
    }
}

fn divide_results(numerator: EvalResult, denominator: EvalResult) -> EvalResult {
    match numerator {
        EvalResult::Value(numerator_value) => match denominator {
            EvalResult::Value(denominator_value) => {
                if denominator_value == 0 {
                    EvalResult::DivisionByZero
                } else {
                    EvalResult::Value(numerator_value / denominator_value)
                }
            }
            EvalResult::DivisionByZero => EvalResult::DivisionByZero,
        },
        EvalResult::DivisionByZero => EvalResult::DivisionByZero,
    }
}

pub fn evaluate(expression: Expr) -> EvalResult {
    match expression {
        Expr::Literal(value) => EvalResult::Value(value),
        Expr::Add(left, right) => add_results(evaluate(*left), evaluate(*right)),
        Expr::Divide(numerator, denominator) => {
            divide_results(evaluate(*numerator), evaluate(*denominator))
        }
        Expr::Negate(inner) => negate_result(evaluate(*inner)),
    }
}
