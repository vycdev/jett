package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve("", ""); !reflect.DeepEqual(got, int64(1)) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("", "a"); !reflect.DeepEqual(got, int64(0)) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("abc", ""); !reflect.DeepEqual(got, int64(4)) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("aaaa", "aa"); !reflect.DeepEqual(got, int64(3)) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("ababa", "aba"); !reflect.DeepEqual(got, int64(2)) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("abc", "abc"); !reflect.DeepEqual(got, int64(1)) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("ab", "abc"); !reflect.DeepEqual(got, int64(0)) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("AaA", "a"); !reflect.DeepEqual(got, int64(1)) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("xxxxx", "xx"); !reflect.DeepEqual(got, int64(4)) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("one one", "one"); !reflect.DeepEqual(got, int64(2)) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve(" ", " "); !reflect.DeepEqual(got, int64(1)) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("abc", "z"); !reflect.DeepEqual(got, int64(0)) {
		t.Fatalf("case 11: got %#v", got)
	}
}
