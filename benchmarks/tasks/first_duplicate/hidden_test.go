package benchmark

import "testing"

func TestFirstDuplicate(t *testing.T) {
	tests := []struct {
		values []int64
		want   MaybeInt
	}{
		{[]int64{}, MaybeInt{Found: false}},
		{[]int64{7}, MaybeInt{Found: false}},
		{[]int64{2, 1, 3, 1, 2}, MaybeInt{Found: true, Value: 1}},
		{[]int64{4, 4, 5, 5}, MaybeInt{Found: true, Value: 4}},
		{[]int64{-2, 0, -2}, MaybeInt{Found: true, Value: -2}},
		{[]int64{0, 1, 0, 1}, MaybeInt{Found: true, Value: 0}},
	}
	for _, test := range tests {
		if got := FirstDuplicate(test.values); got != test.want {
			t.Fatalf("got %#v, want %#v", got, test.want)
		}
	}
}
