package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, "\"\"") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("a"); !reflect.DeepEqual(got, "\"a\"") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("\""); !reflect.DeepEqual(got, "\"\\\"\"") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("\\"); !reflect.DeepEqual(got, "\"\\\\\"") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("\n"); !reflect.DeepEqual(got, "\"\\n\"") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("\t"); !reflect.DeepEqual(got, "\"\\t\"") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("\r"); !reflect.DeepEqual(got, "\"\\r\"") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("\b"); !reflect.DeepEqual(got, "\"\\b\"") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("\f"); !reflect.DeepEqual(got, "\"\\f\"") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("/"); !reflect.DeepEqual(got, "\"/\"") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("a\"b\\c"); !reflect.DeepEqual(got, "\"a\\\"b\\\\c\"") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("x\ny\tz"); !reflect.DeepEqual(got, "\"x\\ny\\tz\"") {
		t.Fatalf("case 11: got %#v", got)
	}
}
