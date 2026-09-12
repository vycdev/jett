package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve(" "); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("a"); !reflect.DeepEqual(got, "a") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("a b"); !reflect.DeepEqual(got, "b a") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve(" a  b c "); !reflect.DeepEqual(got, "c b a") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("one two three"); !reflect.DeepEqual(got, "three two one") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("a a b"); !reflect.DeepEqual(got, "b a a") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("! ?"); !reflect.DeepEqual(got, "? !") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("a\tb c"); !reflect.DeepEqual(got, "c a\tb") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("word  "); !reflect.DeepEqual(got, "word") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("  x"); !reflect.DeepEqual(got, "x") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("ab cd ef gh"); !reflect.DeepEqual(got, "gh ef cd ab") {
		t.Fatalf("case 11: got %#v", got)
	}
}
