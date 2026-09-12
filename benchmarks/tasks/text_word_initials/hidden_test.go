package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("  "); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("alice"); !reflect.DeepEqual(got, "A") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("alice bob"); !reflect.DeepEqual(got, "AB") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("  alice   bob "); !reflect.DeepEqual(got, "AB") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("a b c"); !reflect.DeepEqual(got, "ABC") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("1 one"); !reflect.DeepEqual(got, "1O") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("! bang"); !reflect.DeepEqual(got, "!B") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("mixed CASE"); !reflect.DeepEqual(got, "MC") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("a\tb c"); !reflect.DeepEqual(got, "AC") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("z Z"); !reflect.DeepEqual(got, "ZZ") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("foo-bar baz"); !reflect.DeepEqual(got, "FB") {
		t.Fatalf("case 11: got %#v", got)
	}
}
