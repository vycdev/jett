package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, TextRejected{Error: Empty}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("  "); !reflect.DeepEqual(got, TextRejected{Error: Empty}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("a"); !reflect.DeepEqual(got, TextAccepted{Value: "a"}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("a bb c"); !reflect.DeepEqual(got, TextAccepted{Value: "bb"}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("ab cd"); !reflect.DeepEqual(got, TextAccepted{Value: "ab"}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve(" xxx yy "); !reflect.DeepEqual(got, TextAccepted{Value: "xxx"}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("a\tb zz"); !reflect.DeepEqual(got, TextAccepted{Value: "a\tb"}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("! ??"); !reflect.DeepEqual(got, TextAccepted{Value: "??"}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("long short"); !reflect.DeepEqual(got, TextAccepted{Value: "short"}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("same same"); !reflect.DeepEqual(got, TextAccepted{Value: "same"}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("   end"); !reflect.DeepEqual(got, TextAccepted{Value: "end"}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("ab abc abcd"); !reflect.DeepEqual(got, TextAccepted{Value: "abcd"}) {
		t.Fatalf("case 11: got %#v", got)
	}
}
