package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Covered: 0, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 5}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 1}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 11, End: 15}}, 2)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 2}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 8}, Entry{Start: 6, End: 12}}, 3)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 3}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 13}, Entry{Start: 0, End: 2}, Entry{Start: 1, End: 6}}, 4)
		expected := Report{Covered: 4, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 6}, Entry{Start: 5, End: 10}, Entry{Start: 7, End: 8}, Entry{Start: 6, End: 13}}, 5)
		expected := Report{Covered: 2, Gaps: 1, LongestGap: 3}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 5}, Entry{Start: 9, End: 12}, Entry{Start: 6, End: 7}, Entry{Start: 4, End: 5}, Entry{Start: 8, End: 15}}, 6)
		expected := Report{Covered: 5, Gaps: 1, LongestGap: 1}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 8}, Entry{Start: 7, End: 13}, Entry{Start: 5, End: 9}, Entry{Start: 10, End: 14}, Entry{Start: 9, End: 11}, Entry{Start: 0, End: 2}}, 7)
		expected := Report{Covered: 7, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 5}, Entry{Start: 0, End: 1}, Entry{Start: 6, End: 12}, Entry{Start: 11, End: 12}, Entry{Start: 7, End: 14}, Entry{Start: 5, End: 12}, Entry{Start: 0, End: 3}}, 8)
		expected := Report{Covered: 8, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 14}, Entry{Start: 10, End: 17}, Entry{Start: 7, End: 8}, Entry{Start: 9, End: 11}, Entry{Start: 11, End: 12}, Entry{Start: 9, End: 12}, Entry{Start: 2, End: 3}, Entry{Start: 3, End: 9}}, 1)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 1}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 2, End: 9}, Entry{Start: 5, End: 7}, Entry{Start: 4, End: 6}, Entry{Start: 4, End: 11}, Entry{Start: 5, End: 9}, Entry{Start: 10, End: 11}, Entry{Start: 0, End: 4}, Entry{Start: 6, End: 11}, Entry{Start: 3, End: 4}}, 2)
		expected := Report{Covered: 2, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 7, End: 8}, Entry{Start: 2, End: 7}, Entry{Start: 1, End: 7}, Entry{Start: 11, End: 14}, Entry{Start: 9, End: 12}, Entry{Start: 5, End: 8}, Entry{Start: 8, End: 11}, Entry{Start: 2, End: 4}, Entry{Start: 4, End: 6}, Entry{Start: 8, End: 11}}, 3)
		expected := Report{Covered: 2, Gaps: 1, LongestGap: 1}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 6, End: 12}, Entry{Start: 0, End: 2}, Entry{Start: 7, End: 9}, Entry{Start: 4, End: 5}, Entry{Start: 6, End: 8}, Entry{Start: 8, End: 14}, Entry{Start: 9, End: 14}, Entry{Start: 9, End: 14}, Entry{Start: 3, End: 7}, Entry{Start: 2, End: 8}, Entry{Start: 7, End: 11}}, 4)
		expected := Report{Covered: 4, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 9, End: 12}, Entry{Start: 5, End: 12}, Entry{Start: 3, End: 8}, Entry{Start: 5, End: 7}, Entry{Start: 4, End: 6}, Entry{Start: 9, End: 10}, Entry{Start: 1, End: 8}, Entry{Start: 11, End: 15}, Entry{Start: 6, End: 10}, Entry{Start: 8, End: 9}, Entry{Start: 1, End: 6}, Entry{Start: 6, End: 12}}, 5)
		expected := Report{Covered: 4, Gaps: 1, LongestGap: 1}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 6}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 3}}, 7)
		expected := Report{Covered: 2, Gaps: 2, LongestGap: 4}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 4}, Entry{Start: 2, End: 5}}, 8)
		expected := Report{Covered: 5, Gaps: 1, LongestGap: 3}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 8, End: 10}, Entry{Start: 7, End: 10}, Entry{Start: 7, End: 9}}, 1)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 1}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 4, End: 9}, Entry{Start: 8, End: 12}, Entry{Start: 0, End: 3}, Entry{Start: 8, End: 15}}, 2)
		expected := Report{Covered: 2, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 12}, Entry{Start: 10, End: 11}, Entry{Start: 6, End: 9}, Entry{Start: 10, End: 14}, Entry{Start: 8, End: 9}}, 3)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 3}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 5, End: 9}, Entry{Start: 3, End: 7}, Entry{Start: 4, End: 10}, Entry{Start: 9, End: 16}, Entry{Start: 10, End: 12}, Entry{Start: 9, End: 16}}, 4)
		expected := Report{Covered: 1, Gaps: 1, LongestGap: 3}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 8}, Entry{Start: 3, End: 5}, Entry{Start: 0, End: 6}, Entry{Start: 2, End: 9}, Entry{Start: 8, End: 13}, Entry{Start: 8, End: 10}, Entry{Start: 10, End: 11}}, 5)
		expected := Report{Covered: 5, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 7, End: 14}, Entry{Start: 8, End: 15}, Entry{Start: 2, End: 9}, Entry{Start: 10, End: 11}, Entry{Start: 3, End: 5}, Entry{Start: 5, End: 7}, Entry{Start: 10, End: 12}, Entry{Start: 4, End: 9}}, 6)
		expected := Report{Covered: 4, Gaps: 1, LongestGap: 2}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 3}, Entry{Start: 3, End: 5}}, 5)
		expected := Report{Covered: 5, Gaps: 0, LongestGap: 0}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 9, End: 12}}, 5)
		expected := Report{Covered: 0, Gaps: 1, LongestGap: 5}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 1}, Entry{Start: 4, End: 5}}, 5)
		expected := Report{Covered: 2, Gaps: 1, LongestGap: 3}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 3}, Entry{Start: 2, End: 4}, Entry{Start: 6, End: 9}}, 8)
		expected := Report{Covered: 5, Gaps: 2, LongestGap: 2}
		if actual != expected {
			t.Fatalf("fixture 27: got %+v expected %+v", actual, expected)
		}
	}
}
