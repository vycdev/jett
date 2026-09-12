package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve(" "); !reflect.DeepEqual(got, " ") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("HELLO"); !reflect.DeepEqual(got, "Hello") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("hello WORLD"); !reflect.DeepEqual(got, "Hello World") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("  a  B "); !reflect.DeepEqual(got, "  A  B ") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("a-b"); !reflect.DeepEqual(got, "A-b") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("1ABC"); !reflect.DeepEqual(got, "1abc") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("!HELLO"); !reflect.DeepEqual(got, "!hello") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("x\tY"); !reflect.DeepEqual(got, "X\ty") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("a b c"); !reflect.DeepEqual(got, "A B C") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("MiXeD"); !reflect.DeepEqual(got, "Mixed") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("A  B  C"); !reflect.DeepEqual(got, "A  B  C") {
		t.Fatalf("case 11: got %#v", got)
	}
}
