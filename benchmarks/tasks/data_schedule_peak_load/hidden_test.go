package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Peak: 0, Earliest: (-1), OverloadedStarts: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Peak: 0, Earliest: (-1), OverloadedStarts: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Peak: 0, Earliest: (-1), OverloadedStarts: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 11, End: 15, Demand: 3}}, 2)
		expected := Report{Peak: 3, Earliest: 11, OverloadedStarts: 1}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 8, Demand: 3}, Entry{Start: 6, End: 12, Demand: 5}}, 3)
		expected := Report{Peak: 8, Earliest: 6, OverloadedStarts: 1}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 13, Demand: 1}, Entry{Start: 0, End: 2, Demand: 1}, Entry{Start: 1, End: 6, Demand: 4}}, 4)
		expected := Report{Peak: 5, Earliest: 1, OverloadedStarts: 1}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 6, Demand: 3}, Entry{Start: 5, End: 10, Demand: 2}, Entry{Start: 7, End: 8, Demand: 2}, Entry{Start: 6, End: 13, Demand: 3}}, 5)
		expected := Report{Peak: 7, Earliest: 7, OverloadedStarts: 1}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 5, Demand: 5}, Entry{Start: 9, End: 12, Demand: 2}, Entry{Start: 6, End: 7, Demand: 2}, Entry{Start: 4, End: 5, Demand: 4}, Entry{Start: 8, End: 15, Demand: 2}}, 6)
		expected := Report{Peak: 9, Earliest: 4, OverloadedStarts: 1}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 8, Demand: 5}, Entry{Start: 7, End: 13, Demand: 5}, Entry{Start: 5, End: 9, Demand: 1}, Entry{Start: 10, End: 14, Demand: 5}, Entry{Start: 9, End: 11, Demand: 5}, Entry{Start: 0, End: 2, Demand: 3}}, 7)
		expected := Report{Peak: 15, Earliest: 10, OverloadedStarts: 4}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 5, Demand: 2}, Entry{Start: 0, End: 1, Demand: 4}, Entry{Start: 6, End: 12, Demand: 4}, Entry{Start: 11, End: 12, Demand: 5}, Entry{Start: 7, End: 14, Demand: 2}, Entry{Start: 5, End: 12, Demand: 1}, Entry{Start: 0, End: 3, Demand: 2}}, 8)
		expected := Report{Peak: 12, Earliest: 11, OverloadedStarts: 1}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 14, Demand: 2}, Entry{Start: 10, End: 17, Demand: 1}, Entry{Start: 7, End: 8, Demand: 3}, Entry{Start: 9, End: 11, Demand: 4}, Entry{Start: 11, End: 12, Demand: 4}, Entry{Start: 9, End: 12, Demand: 4}, Entry{Start: 2, End: 3, Demand: 3}, Entry{Start: 3, End: 9, Demand: 3}}, 1)
		expected := Report{Peak: 11, Earliest: 10, OverloadedStarts: 6}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 2, End: 9, Demand: 5}, Entry{Start: 5, End: 7, Demand: 4}, Entry{Start: 4, End: 6, Demand: 3}, Entry{Start: 4, End: 11, Demand: 5}, Entry{Start: 5, End: 9, Demand: 2}, Entry{Start: 10, End: 11, Demand: 3}, Entry{Start: 0, End: 4, Demand: 1}, Entry{Start: 6, End: 11, Demand: 3}, Entry{Start: 3, End: 4, Demand: 1}}, 2)
		expected := Report{Peak: 19, Earliest: 5, OverloadedStarts: 6}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 7, End: 8, Demand: 1}, Entry{Start: 2, End: 7, Demand: 1}, Entry{Start: 1, End: 7, Demand: 2}, Entry{Start: 11, End: 14, Demand: 4}, Entry{Start: 9, End: 12, Demand: 1}, Entry{Start: 5, End: 8, Demand: 5}, Entry{Start: 8, End: 11, Demand: 3}, Entry{Start: 2, End: 4, Demand: 1}, Entry{Start: 4, End: 6, Demand: 3}, Entry{Start: 8, End: 11, Demand: 5}}, 3)
		expected := Report{Peak: 11, Earliest: 5, OverloadedStarts: 7}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 6, End: 12, Demand: 4}, Entry{Start: 0, End: 2, Demand: 4}, Entry{Start: 7, End: 9, Demand: 1}, Entry{Start: 4, End: 5, Demand: 5}, Entry{Start: 6, End: 8, Demand: 2}, Entry{Start: 8, End: 14, Demand: 5}, Entry{Start: 9, End: 14, Demand: 2}, Entry{Start: 9, End: 14, Demand: 2}, Entry{Start: 3, End: 7, Demand: 2}, Entry{Start: 2, End: 8, Demand: 2}, Entry{Start: 7, End: 11, Demand: 3}}, 4)
		expected := Report{Peak: 16, Earliest: 9, OverloadedStarts: 5}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 9, End: 12, Demand: 3}, Entry{Start: 5, End: 12, Demand: 2}, Entry{Start: 3, End: 8, Demand: 1}, Entry{Start: 5, End: 7, Demand: 2}, Entry{Start: 4, End: 6, Demand: 2}, Entry{Start: 9, End: 10, Demand: 2}, Entry{Start: 1, End: 8, Demand: 4}, Entry{Start: 11, End: 15, Demand: 1}, Entry{Start: 6, End: 10, Demand: 2}, Entry{Start: 8, End: 9, Demand: 4}, Entry{Start: 1, End: 6, Demand: 4}, Entry{Start: 6, End: 12, Demand: 3}}, 5)
		expected := Report{Peak: 15, Earliest: 5, OverloadedStarts: 8}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Peak: 0, Earliest: (-1), OverloadedStarts: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 3, Demand: 4}}, 7)
		expected := Report{Peak: 4, Earliest: 1, OverloadedStarts: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 4, Demand: 4}, Entry{Start: 2, End: 5, Demand: 1}}, 8)
		expected := Report{Peak: 5, Earliest: 2, OverloadedStarts: 0}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 8, End: 10, Demand: 4}, Entry{Start: 7, End: 10, Demand: 1}, Entry{Start: 7, End: 9, Demand: 2}}, 1)
		expected := Report{Peak: 7, Earliest: 8, OverloadedStarts: 2}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 4, End: 9, Demand: 3}, Entry{Start: 8, End: 12, Demand: 5}, Entry{Start: 0, End: 3, Demand: 3}, Entry{Start: 8, End: 15, Demand: 5}}, 2)
		expected := Report{Peak: 13, Earliest: 8, OverloadedStarts: 3}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 12, Demand: 1}, Entry{Start: 10, End: 11, Demand: 4}, Entry{Start: 6, End: 9, Demand: 3}, Entry{Start: 10, End: 14, Demand: 1}, Entry{Start: 8, End: 9, Demand: 1}}, 3)
		expected := Report{Peak: 6, Earliest: 10, OverloadedStarts: 2}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 5, End: 9, Demand: 1}, Entry{Start: 3, End: 7, Demand: 3}, Entry{Start: 4, End: 10, Demand: 1}, Entry{Start: 9, End: 16, Demand: 5}, Entry{Start: 10, End: 12, Demand: 5}, Entry{Start: 9, End: 16, Demand: 1}}, 4)
		expected := Report{Peak: 11, Earliest: 10, OverloadedStarts: 3}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 8, Demand: 2}, Entry{Start: 3, End: 5, Demand: 3}, Entry{Start: 0, End: 6, Demand: 5}, Entry{Start: 2, End: 9, Demand: 4}, Entry{Start: 8, End: 13, Demand: 1}, Entry{Start: 8, End: 10, Demand: 1}, Entry{Start: 10, End: 11, Demand: 1}}, 5)
		expected := Report{Peak: 14, Earliest: 3, OverloadedStarts: 3}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 7, End: 14, Demand: 3}, Entry{Start: 8, End: 15, Demand: 1}, Entry{Start: 2, End: 9, Demand: 2}, Entry{Start: 10, End: 11, Demand: 3}, Entry{Start: 3, End: 5, Demand: 1}, Entry{Start: 5, End: 7, Demand: 2}, Entry{Start: 10, End: 12, Demand: 3}, Entry{Start: 4, End: 9, Demand: 4}}, 6)
		expected := Report{Peak: 10, Earliest: 8, OverloadedStarts: 5}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 2, Demand: 3}, Entry{Start: 2, End: 4, Demand: 3}}, 3)
		expected := Report{Peak: 3, Earliest: 0, OverloadedStarts: 0}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 2, End: 4, Demand: 5}, Entry{Start: 0, End: 2, Demand: 5}, Entry{Start: 0, End: 2, Demand: 1}}, 5)
		expected := Report{Peak: 6, Earliest: 0, OverloadedStarts: 1}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 4, End: 8, Demand: 2}, Entry{Start: 0, End: 4, Demand: 3}, Entry{Start: 2, End: 6, Demand: 4}, Entry{Start: 2, End: 3, Demand: 1}}, 5)
		expected := Report{Peak: 8, Earliest: 2, OverloadedStarts: 2}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
}
