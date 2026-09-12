package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, TextAccepted{Value: ""}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("1A"); !reflect.DeepEqual(got, TextAccepted{Value: "A"}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("3A2B"); !reflect.DeepEqual(got, TextAccepted{Value: "AAABB"}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("1A1A"); !reflect.DeepEqual(got, TextAccepted{Value: "AA"}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("100Z"); !reflect.DeepEqual(got, TextAccepted{Value: "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("0A"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("01A"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("101A"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("2a"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("A"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("12"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("60A41B"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 11: got %#v", got)
	}
	if got := solve("2A0B"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 12: got %#v", got)
	}
}
