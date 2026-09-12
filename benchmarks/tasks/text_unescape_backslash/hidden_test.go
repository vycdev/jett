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
	if got := solve("\\n"); !reflect.DeepEqual(got, TextAccepted{Value: "\n"}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("\\t\\r"); !reflect.DeepEqual(got, TextAccepted{Value: "\t\r"}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("\\\\"); !reflect.DeepEqual(got, TextAccepted{Value: "\\"}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("\\\""); !reflect.DeepEqual(got, TextAccepted{Value: "\""}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("a\\nb"); !reflect.DeepEqual(got, TextAccepted{Value: "a\nb"}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("\\"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("x\\q"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("\\0"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("\\\\n"); !reflect.DeepEqual(got, TextAccepted{Value: "\\n"}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("a\nb"); !reflect.DeepEqual(got, TextAccepted{Value: "a\nb"}) {
		t.Fatalf("case 11: got %#v", got)
	}
}
