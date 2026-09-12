package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve("", 5); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("   ", 3); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("a b c", 3); !reflect.DeepEqual(got, "a b\nc") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("a b c", 1); !reflect.DeepEqual(got, "a\nb\nc") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("longword x", 3); !reflect.DeepEqual(got, "longword\nx") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("one two three", 7); !reflect.DeepEqual(got, "one two\nthree") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve(" one  two ", 80); !reflect.DeepEqual(got, "one two") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("ab cd ef", 5); !reflect.DeepEqual(got, "ab cd\nef") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("abc d", 3); !reflect.DeepEqual(got, "abc\nd") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("a bc", 4); !reflect.DeepEqual(got, "a bc") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("a bc", 3); !reflect.DeepEqual(got, "a\nbc") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("x longword y", 3); !reflect.DeepEqual(got, "x\nlongword\ny") {
		t.Fatalf("case 11: got %#v", got)
	}
}
