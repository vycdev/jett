package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, TextRejected{Error: Empty}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("0"); !reflect.DeepEqual(got, TextAccepted{Value: 0}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("1"); !reflect.DeepEqual(got, TextAccepted{Value: 1}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("00101"); !reflect.DeepEqual(got, TextAccepted{Value: 5}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("1111111111111111"); !reflect.DeepEqual(got, TextAccepted{Value: 65535}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("1000000000000000"); !reflect.DeepEqual(got, TextAccepted{Value: 32768}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("00000000000000000"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("2"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("10x"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve(" 1"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("+1"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("101010"); !reflect.DeepEqual(got, TextAccepted{Value: 42}) {
		t.Fatalf("case 11: got %#v", got)
	}
	if got := solve("xxxxxxxxxxxxxxxxx"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 12: got %#v", got)
	}
}
