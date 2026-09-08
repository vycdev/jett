package benchmark

import "testing"

func TestSignedGCD(t *testing.T) {
	cases := []struct{ a, b, want int64 }{
		{0, 0, 0}, {54, 24, 6}, {-54, 24, 6}, {54, -24, 6},
		{-7, -13, 1}, {0, 42, 42}, {270, 192, 6},
	}
	for _, tc := range cases {
		if got := SignedGCD(tc.a, tc.b); got != tc.want {
			t.Fatalf("got %d, want %d", got, tc.want)
		}
	}
}
