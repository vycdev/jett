package benchmark

import "testing"

func TestBoundedWeightedSum(t *testing.T) {
	cases := []struct {
		values []int64
		cap    int64
		want   int64
	}{
		{nil, 5, 0},
		{[]int64{1, 2, 3}, 10, 14},
		{[]int64{20, -20, 3}, 10, -1},
		{[]int64{-1, 2, -3, 4}, 2, 5},
		{[]int64{9, -4}, 0, 0},
	}
	for _, tc := range cases {
		if got := BoundedWeightedSum(tc.values, tc.cap); got != tc.want {
			t.Fatalf("got %d, want %d", got, tc.want)
		}
	}
}
