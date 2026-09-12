package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("a"); !reflect.DeepEqual(got, "a") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("A"); !reflect.DeepEqual(got, "a") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("camelCase"); !reflect.DeepEqual(got, "camel_case") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("HTTPServer"); !reflect.DeepEqual(got, "http_server") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("XML"); !reflect.DeepEqual(got, "xml") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("parseURLValue"); !reflect.DeepEqual(got, "parse_url_value") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("x2Y"); !reflect.DeepEqual(got, "x2_y") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("ABc"); !reflect.DeepEqual(got, "a_bc") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("oneTwoThree"); !reflect.DeepEqual(got, "one_two_three") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("v123"); !reflect.DeepEqual(got, "v123") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("ABCdEF"); !reflect.DeepEqual(got, "ab_cd_ef") {
		t.Fatalf("case 11: got %#v", got)
	}
}
