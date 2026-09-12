package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("! ?"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("a"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("ab"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("Aa"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("Race car!"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("A man, a plan, a canal: Panama"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("12 21"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("12a21"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("12a22"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("0P"); !reflect.DeepEqual(got, false) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("ab\tBA"); !reflect.DeepEqual(got, true) {
		t.Fatalf("case 11: got %#v", got)
	}
}
