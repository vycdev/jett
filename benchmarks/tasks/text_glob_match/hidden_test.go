package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve("", ""); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("", "*"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("", "?"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("abc", "a?c"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("abc", "a*c"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("abbbc", "a*b*c"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("abc", "*d"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("abcd", "a*d?"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("abcdef", "*?c*f"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("aaaab", "a*b"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("abc", "**a**?**c**"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("ab", "a"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 11: got %#v", got)
	}
	if got := solve("a.b", "a.b"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 12: got %#v", got)
	}
	if got := solve("ABC", "abc"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 13: got %#v", got)
	}
}
