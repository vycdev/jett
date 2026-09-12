package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, TextAccepted{Value: ""}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("abc"); !reflect.DeepEqual(got, TextAccepted{Value: "abc"}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("a+b"); !reflect.DeepEqual(got, TextAccepted{Value: "a+b"}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("%41%20%7e"); !reflect.DeepEqual(got, TextAccepted{Value: "A ~"}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("%25"); !reflect.DeepEqual(got, TextAccepted{Value: "%"}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("%"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("%2"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("%GG"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("%00"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("%7F"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("%ff"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("%00%G0"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 11: got %#v", got)
	}
	if got := solve("%G0%00"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 12: got %#v", got)
	}
	if got := solve("x%2fy"); !reflect.DeepEqual(got, TextAccepted{Value: "x/y"}) {
		t.Fatalf("case 13: got %#v", got)
	}
}
