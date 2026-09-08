package benchmark

import (
	"reflect"
	"testing"
)

func TestRecursiveExpression(t *testing.T) {
	literal := func(value int64) Expr { return Literal{Value: value} }
	tests := []struct {
		expression Expr
		want       EvalResult
	}{
		{literal(7), ValueResult{Value: 7}},
		{Add{Left: literal(2), Right: literal(3)}, ValueResult{Value: 5}},
		{Negate{Inner: Add{Left: literal(4), Right: literal(5)}}, ValueResult{Value: -9}},
		{Divide{Numerator: literal(8), Denominator: literal(2)}, ValueResult{Value: 4}},
		{Divide{Numerator: literal(8), Denominator: literal(0)}, DivisionByZero{}},
		{Divide{Numerator: literal(10), Denominator: Divide{Numerator: literal(1), Denominator: literal(0)}}, DivisionByZero{}},
		{Add{Left: Divide{Numerator: literal(1), Denominator: literal(0)}, Right: literal(5)}, DivisionByZero{}},
		{Divide{Numerator: literal(21), Denominator: Negate{Inner: literal(3)}}, ValueResult{Value: -7}},
		{Divide{Numerator: literal(-7), Denominator: literal(2)}, ValueResult{Value: -3}},
		{Divide{Numerator: literal(7), Denominator: literal(-2)}, ValueResult{Value: -3}},
	}
	for _, test := range tests {
		if got := Evaluate(test.expression); !reflect.DeepEqual(got, test.want) {
			t.Fatalf("got %#v, want %#v", got, test.want)
		}
	}
}
