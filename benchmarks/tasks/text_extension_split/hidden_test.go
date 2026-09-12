package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, TextPair{First: "", Second: ""}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("a"); !reflect.DeepEqual(got, TextPair{First: "a", Second: ""}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("a.txt"); !reflect.DeepEqual(got, TextPair{First: "a", Second: "txt"}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("a.tar.gz"); !reflect.DeepEqual(got, TextPair{First: "a.tar", Second: "gz"}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve(".env"); !reflect.DeepEqual(got, TextPair{First: ".env", Second: ""}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve(".env.local"); !reflect.DeepEqual(got, TextPair{First: ".env", Second: "local"}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("a."); !reflect.DeepEqual(got, TextPair{First: "a", Second: ""}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("."); !reflect.DeepEqual(got, TextPair{First: ".", Second: ""}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve(".."); !reflect.DeepEqual(got, TextPair{First: ".", Second: ""}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("..."); !reflect.DeepEqual(got, TextPair{First: "..", Second: ""}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("A.TXT"); !reflect.DeepEqual(got, TextPair{First: "A", Second: "TXT"}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve(" a .x"); !reflect.DeepEqual(got, TextPair{First: " a ", Second: "x"}) {
		t.Fatalf("case 11: got %#v", got)
	}
}
