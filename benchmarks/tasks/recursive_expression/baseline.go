package benchmark

type Expr interface{ isExpr() }

type Literal struct{ Value int64 }
type Add struct{ Left, Right Expr }
type Divide struct{ Numerator, Denominator Expr }
type Negate struct{ Inner Expr }

func (Literal) isExpr() {}
func (Add) isExpr()     {}
func (Divide) isExpr()  {}
func (Negate) isExpr()  {}

type EvalResult interface{ isEvalResult() }

type ValueResult struct{ Value int64 }
type DivisionByZero struct{}

func (ValueResult) isEvalResult()    {}
func (DivisionByZero) isEvalResult() {}

func negateResult(result EvalResult) EvalResult {
	switch result := result.(type) {
	case ValueResult:
		return ValueResult{Value: -result.Value}
	case DivisionByZero:
		return result
	}
	return DivisionByZero{}
}

func addResults(left, right EvalResult) EvalResult {
	switch left := left.(type) {
	case ValueResult:
		switch right := right.(type) {
		case ValueResult:
			return ValueResult{Value: left.Value + right.Value}
		case DivisionByZero:
			return right
		}
	case DivisionByZero:
		return left
	}
	return DivisionByZero{}
}

func divideResults(numerator, denominator EvalResult) EvalResult {
	switch numerator := numerator.(type) {
	case ValueResult:
		switch denominator := denominator.(type) {
		case ValueResult:
			if denominator.Value == 0 {
				return DivisionByZero{}
			}
			return ValueResult{Value: numerator.Value / denominator.Value}
		case DivisionByZero:
			return denominator
		}
	case DivisionByZero:
		return numerator
	}
	return DivisionByZero{}
}

func Evaluate(expression Expr) EvalResult {
	switch expression := expression.(type) {
	case Literal:
		return ValueResult{Value: expression.Value}
	case Add:
		return addResults(Evaluate(expression.Left), Evaluate(expression.Right))
	case Divide:
		return divideResults(Evaluate(expression.Numerator), Evaluate(expression.Denominator))
	case Negate:
		return negateResult(Evaluate(expression.Inner))
	}
	return DivisionByZero{}
}
