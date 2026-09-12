package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve("a=b"); !reflect.DeepEqual(got, TextPair{First: "a", Second: "b"}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve(" a = b "); !reflect.DeepEqual(got, TextPair{First: "a", Second: "b"}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("a="); !reflect.DeepEqual(got, TextPair{First: "a", Second: ""}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("a=  "); !reflect.DeepEqual(got, TextPair{First: "a", Second: ""}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("a=b=c"); !reflect.DeepEqual(got, TextPair{First: "a", Second: "b=c"}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("a=="); !reflect.DeepEqual(got, TextPair{First: "a", Second: "="}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("x y=z z"); !reflect.DeepEqual(got, TextPair{First: "x y", Second: "z z"}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("\ta\t=\tb\t"); !reflect.DeepEqual(got, TextPair{First: "\ta\t", Second: "\tb\t"}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("x= a  b "); !reflect.DeepEqual(got, TextPair{First: "x", Second: "a  b"}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("KEY=value"); !reflect.DeepEqual(got, TextPair{First: "KEY", Second: "value"}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("a= = "); !reflect.DeepEqual(got, TextPair{First: "a", Second: "="}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("  k  = v=x "); !reflect.DeepEqual(got, TextPair{First: "k", Second: "v=x"}) {
		t.Fatalf("case 11: got %#v", got)
	}
}
