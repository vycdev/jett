package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("_"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("a"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("A0_z"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("0a"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("a-b"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("a b"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("if"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("__"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("a\n"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("$a"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("z9"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 11: got %#v", got)
	}
}
