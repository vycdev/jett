package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, TextRejected{Error: Empty}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("0"); !reflect.DeepEqual(got, TextAccepted{Value: "0"}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("-000"); !reflect.DeepEqual(got, TextAccepted{Value: "0"}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("+0012"); !reflect.DeepEqual(got, TextAccepted{Value: "12"}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("-012"); !reflect.DeepEqual(got, TextAccepted{Value: "-12"}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("123"); !reflect.DeepEqual(got, TextAccepted{Value: "123"}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("+"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("--1"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve(" 1"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("1.0"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("0x0"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999"); !reflect.DeepEqual(got, TextAccepted{Value: "9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999"}) {
		t.Fatalf("case 11: got %#v", got)
	}
	if got := solve("-0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001"); !reflect.DeepEqual(got, TextAccepted{Value: "-1"}) {
		t.Fatalf("case 12: got %#v", got)
	}
}
