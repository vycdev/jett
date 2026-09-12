package benchmark

import (
	"reflect"
	"testing"
)

func TestHidden(t *testing.T) {
	if got := solve(""); !reflect.DeepEqual(got, "") {
		t.Fatalf("case 0: got %#v", got)
	}
	if got := solve("abc"); !reflect.DeepEqual(got, "abc") {
		t.Fatalf("case 1: got %#v", got)
	}
	if got := solve("&"); !reflect.DeepEqual(got, "&amp;") {
		t.Fatalf("case 2: got %#v", got)
	}
	if got := solve("<>"); !reflect.DeepEqual(got, "&lt;&gt;") {
		t.Fatalf("case 3: got %#v", got)
	}
	if got := solve("\"'"); !reflect.DeepEqual(got, "&quot;&#39;") {
		t.Fatalf("case 4: got %#v", got)
	}
	if got := solve("&amp;"); !reflect.DeepEqual(got, "&amp;amp;") {
		t.Fatalf("case 5: got %#v", got)
	}
	if got := solve("a&b<c"); !reflect.DeepEqual(got, "a&amp;b&lt;c") {
		t.Fatalf("case 6: got %#v", got)
	}
	if got := solve("&&"); !reflect.DeepEqual(got, "&amp;&amp;") {
		t.Fatalf("case 7: got %#v", got)
	}
	if got := solve("\n\t"); !reflect.DeepEqual(got, "\n\t") {
		t.Fatalf("case 8: got %#v", got)
	}
	if got := solve("/a=1"); !reflect.DeepEqual(got, "/a=1") {
		t.Fatalf("case 9: got %#v", got)
	}
	if got := solve("<a x=\"b\">"); !reflect.DeepEqual(got, "&lt;a x=&quot;b&quot;&gt;") {
		t.Fatalf("case 10: got %#v", got)
	}
	if got := solve("'&'"); !reflect.DeepEqual(got, "&#39;&amp;&#39;") {
		t.Fatalf("case 11: got %#v", got)
	}
}
