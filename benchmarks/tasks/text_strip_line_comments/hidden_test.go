package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("abc"); !reflect.DeepEqual(got, "abc") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("#x"); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("a#b"); !reflect.DeepEqual(got, "a") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("a#b\nc"); !reflect.DeepEqual(got, "a\nc") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("\"#x\"#y"); !reflect.DeepEqual(got, "\"#x\"") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("#a\n#b\n"); !reflect.DeepEqual(got, "\n\n") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("a # x"); !reflect.DeepEqual(got, "a ") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("\"a\\\"#b\"#c"); !reflect.DeepEqual(got, "\"a\\\"#b\"") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("\\#x"); !reflect.DeepEqual(got, "\\") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("\"#open"); !reflect.DeepEqual(got, "\"#open") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("a\n#x\nb"); !reflect.DeepEqual(got, "a\n\nb") {
		t.Fatalf("case 11: got %#v", got)
	}
}
