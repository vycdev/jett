package benchmark

import "testing"

func TestTriangleKind(t *testing.T) {
	cases := []struct {
		a, b, c int64
		want    string
	}{
		{3, 3, 3, "equilateral"}, {5, 5, 8, "isosceles"},
		{3, 4, 5, "scalene"}, {1, 2, 3, "invalid"},
		{0, 4, 4, "invalid"}, {-1, 2, 2, "invalid"},
		{10, 3, 3, "invalid"},
	}
	for _, tc := range cases {
		if got := TriangleKind(tc.a, tc.b, tc.c); got != tc.want {
			t.Fatalf("got %q, want %q", got, tc.want)
		}
	}
}
