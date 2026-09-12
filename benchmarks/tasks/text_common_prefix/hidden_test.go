package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve("", ""); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("a", ""); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("", "a"); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("abc", "abc"); !reflect.DeepEqual(got, "abc") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("abc", "abd"); !reflect.DeepEqual(got, "ab") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("abc", "ab"); !reflect.DeepEqual(got, "ab") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("ab", "abc"); !reflect.DeepEqual(got, "ab") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("a", "A"); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("foo bar", "foo baz"); !reflect.DeepEqual(got, "foo ba") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve(" x", " y"); !reflect.DeepEqual(got, " ") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("123", "129"); !reflect.DeepEqual(got, "12") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("abc", "zabc"); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 11: got %#v", got)
	}
}
