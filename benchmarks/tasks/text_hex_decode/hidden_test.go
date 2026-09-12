package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, TextAccepted{Value: ""}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("41"); !reflect.DeepEqual(got, TextAccepted{Value: "A"}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("6869"); !reflect.DeepEqual(got, TextAccepted{Value: "hi"}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("207E"); !reflect.DeepEqual(got, TextAccepted{Value: " ~"}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("5c22"); !reflect.DeepEqual(got, TextAccepted{Value: "\\\""}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("0"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("GG"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("00"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("7f"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("ff"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("00g"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("002G"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 11: got %#v", got)
	}
	if got := solve("2G00"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 12: got %#v", got)
	}
}
