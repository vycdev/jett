package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve("0.0.0.0"); !reflect.DeepEqual(got, TextAccepted{Value: 0}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("255.255.255.255"); !reflect.DeepEqual(got, TextAccepted{Value: 4294967295}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("127.0.0.1"); !reflect.DeepEqual(got, TextAccepted{Value: 2130706433}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("192.168.1.1"); !reflect.DeepEqual(got, TextAccepted{Value: 3232235777}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("1.2.3.4"); !reflect.DeepEqual(got, TextAccepted{Value: 16909060}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve(""); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("1.2.3"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("1.2.3.4.5"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("01.2.3.4"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("256.0.0.1"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("1..2.3"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("1.2.3.-1"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 11: got %#v", got)
	}
	if got := solve("1.2.3.4 "); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 12: got %#v", got)
	}
}
