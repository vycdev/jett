package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve("00:00:00"); !reflect.DeepEqual(got, TextAccepted{Value: 0}) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("23:59:59"); !reflect.DeepEqual(got, TextAccepted{Value: 86399}) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("12:34:56"); !reflect.DeepEqual(got, TextAccepted{Value: 45296}) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("01:02:03"); !reflect.DeepEqual(got, TextAccepted{Value: 3723}) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("24:00:00"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("00:60:00"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("00:00:60"); !reflect.DeepEqual(got, TextRejected{Error: Range}) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("0:00:00"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("00-00-00"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("aa:00:00"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("99:xx:00"); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve(""); !reflect.DeepEqual(got, TextRejected{Error: Malformed}) {
		t.Fatalf("case 11: got %#v", got)
	}
}
