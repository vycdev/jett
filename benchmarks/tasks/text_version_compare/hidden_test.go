package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve("1", "1"); !reflect.DeepEqual(got, int64(0)) {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("1", "1.0.0"); !reflect.DeepEqual(got, int64(0)) {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("0.0", "0"); !reflect.DeepEqual(got, int64(0)) {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("1.2", "1.10"); !reflect.DeepEqual(got, int64(-1)) {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("2", "1.999"); !reflect.DeepEqual(got, int64(1)) {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("1.0.1", "1"); !reflect.DeepEqual(got, int64(1)) {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("1", "1.0.1"); !reflect.DeepEqual(got, int64(-1)) {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("999", "998.999"); !reflect.DeepEqual(got, int64(1)) {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("0.1", "0.0.9"); !reflect.DeepEqual(got, int64(1)) {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("1.2.3", "1.2.3"); !reflect.DeepEqual(got, int64(0)) {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("1.2.3", "1.2.4"); !reflect.DeepEqual(got, int64(-1)) {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("1.0.0.0.0.0.0.0", "1"); !reflect.DeepEqual(got, int64(0)) {
		t.Fatalf("case 11: got %#v", got)
	}
}
