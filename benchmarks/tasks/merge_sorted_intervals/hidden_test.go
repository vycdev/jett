package benchmark

import (
	"reflect"
	"testing"
)

func TestMergeSortedIntervals(t *testing.T) {
	tests := []struct {
		input []Interval
		want  []Interval
	}{
		{[]Interval{}, []Interval{}},
		{[]Interval{{Start: 1, End: 3}}, []Interval{{Start: 1, End: 3}}},
		{[]Interval{{Start: 1, End: 3}, {Start: 2, End: 6}, {Start: 8, End: 10}}, []Interval{{Start: 1, End: 6}, {Start: 8, End: 10}}},
		{[]Interval{{Start: 1, End: 10}, {Start: 2, End: 3}, {Start: 10, End: 12}}, []Interval{{Start: 1, End: 12}}},
		{[]Interval{{Start: -5, End: -2}, {Start: -2, End: 0}, {Start: 4, End: 4}}, []Interval{{Start: -5, End: 0}, {Start: 4, End: 4}}},
	}
	for _, test := range tests {
		if got := MergeSortedIntervals(test.input); !reflect.DeepEqual(got, test.want) {
			t.Fatalf("got %#v, want %#v", got, test.want)
		}
	}
}
