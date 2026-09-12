package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{ConflictPairs: 0, AffectedBookings: 0, LongestOverlap: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 11, End: 15, ResourceId: 3}})
		expected := Report{ConflictPairs: 0, AffectedBookings: 0, LongestOverlap: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 8, ResourceId: 3}, Entry{Start: 6, End: 12, ResourceId: 5}})
		expected := Report{ConflictPairs: 0, AffectedBookings: 0, LongestOverlap: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 13, ResourceId: 1}, Entry{Start: 0, End: 2, ResourceId: 1}, Entry{Start: 1, End: 6, ResourceId: 4}})
		expected := Report{ConflictPairs: 0, AffectedBookings: 0, LongestOverlap: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 6, ResourceId: 3}, Entry{Start: 5, End: 10, ResourceId: 2}, Entry{Start: 7, End: 8, ResourceId: 2}, Entry{Start: 6, End: 13, ResourceId: 3}})
		expected := Report{ConflictPairs: 1, AffectedBookings: 2, LongestOverlap: 1}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 5, ResourceId: 5}, Entry{Start: 9, End: 12, ResourceId: 2}, Entry{Start: 6, End: 7, ResourceId: 2}, Entry{Start: 4, End: 5, ResourceId: 4}, Entry{Start: 8, End: 15, ResourceId: 2}})
		expected := Report{ConflictPairs: 1, AffectedBookings: 2, LongestOverlap: 3}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 8, ResourceId: 5}, Entry{Start: 7, End: 13, ResourceId: 5}, Entry{Start: 5, End: 9, ResourceId: 1}, Entry{Start: 10, End: 14, ResourceId: 5}, Entry{Start: 9, End: 11, ResourceId: 5}, Entry{Start: 0, End: 2, ResourceId: 3}})
		expected := Report{ConflictPairs: 4, AffectedBookings: 4, LongestOverlap: 3}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 5, ResourceId: 2}, Entry{Start: 0, End: 1, ResourceId: 4}, Entry{Start: 6, End: 12, ResourceId: 4}, Entry{Start: 11, End: 12, ResourceId: 5}, Entry{Start: 7, End: 14, ResourceId: 2}, Entry{Start: 5, End: 12, ResourceId: 1}, Entry{Start: 0, End: 3, ResourceId: 2}})
		expected := Report{ConflictPairs: 1, AffectedBookings: 2, LongestOverlap: 2}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 14, ResourceId: 2}, Entry{Start: 10, End: 17, ResourceId: 1}, Entry{Start: 7, End: 8, ResourceId: 3}, Entry{Start: 9, End: 11, ResourceId: 4}, Entry{Start: 11, End: 12, ResourceId: 4}, Entry{Start: 9, End: 12, ResourceId: 4}, Entry{Start: 2, End: 3, ResourceId: 3}, Entry{Start: 3, End: 9, ResourceId: 3}})
		expected := Report{ConflictPairs: 3, AffectedBookings: 5, LongestOverlap: 2}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 2, End: 9, ResourceId: 5}, Entry{Start: 5, End: 7, ResourceId: 4}, Entry{Start: 4, End: 6, ResourceId: 3}, Entry{Start: 4, End: 11, ResourceId: 5}, Entry{Start: 5, End: 9, ResourceId: 2}, Entry{Start: 10, End: 11, ResourceId: 3}, Entry{Start: 0, End: 4, ResourceId: 1}, Entry{Start: 6, End: 11, ResourceId: 3}, Entry{Start: 3, End: 4, ResourceId: 1}})
		expected := Report{ConflictPairs: 3, AffectedBookings: 6, LongestOverlap: 5}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 7, End: 8, ResourceId: 1}, Entry{Start: 2, End: 7, ResourceId: 1}, Entry{Start: 1, End: 7, ResourceId: 2}, Entry{Start: 11, End: 14, ResourceId: 4}, Entry{Start: 9, End: 12, ResourceId: 1}, Entry{Start: 5, End: 8, ResourceId: 5}, Entry{Start: 8, End: 11, ResourceId: 3}, Entry{Start: 2, End: 4, ResourceId: 1}, Entry{Start: 4, End: 6, ResourceId: 3}, Entry{Start: 8, End: 11, ResourceId: 5}})
		expected := Report{ConflictPairs: 1, AffectedBookings: 2, LongestOverlap: 2}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 6, End: 12, ResourceId: 4}, Entry{Start: 0, End: 2, ResourceId: 4}, Entry{Start: 7, End: 9, ResourceId: 1}, Entry{Start: 4, End: 5, ResourceId: 5}, Entry{Start: 6, End: 8, ResourceId: 2}, Entry{Start: 8, End: 14, ResourceId: 5}, Entry{Start: 9, End: 14, ResourceId: 2}, Entry{Start: 9, End: 14, ResourceId: 2}, Entry{Start: 3, End: 7, ResourceId: 2}, Entry{Start: 2, End: 8, ResourceId: 2}, Entry{Start: 7, End: 11, ResourceId: 3}})
		expected := Report{ConflictPairs: 4, AffectedBookings: 5, LongestOverlap: 5}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 9, End: 12, ResourceId: 3}, Entry{Start: 5, End: 12, ResourceId: 2}, Entry{Start: 3, End: 8, ResourceId: 1}, Entry{Start: 5, End: 7, ResourceId: 2}, Entry{Start: 4, End: 6, ResourceId: 2}, Entry{Start: 9, End: 10, ResourceId: 2}, Entry{Start: 1, End: 8, ResourceId: 4}, Entry{Start: 11, End: 15, ResourceId: 1}, Entry{Start: 6, End: 10, ResourceId: 2}, Entry{Start: 8, End: 9, ResourceId: 4}, Entry{Start: 1, End: 6, ResourceId: 4}, Entry{Start: 6, End: 12, ResourceId: 3}})
		expected := Report{ConflictPairs: 9, AffectedBookings: 9, LongestOverlap: 5}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 3, ResourceId: 4}})
		expected := Report{ConflictPairs: 0, AffectedBookings: 0, LongestOverlap: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 4, ResourceId: 4}, Entry{Start: 2, End: 5, ResourceId: 1}})
		expected := Report{ConflictPairs: 0, AffectedBookings: 0, LongestOverlap: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 8, End: 10, ResourceId: 4}, Entry{Start: 7, End: 10, ResourceId: 1}, Entry{Start: 7, End: 9, ResourceId: 2}})
		expected := Report{ConflictPairs: 0, AffectedBookings: 0, LongestOverlap: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 4, End: 9, ResourceId: 3}, Entry{Start: 8, End: 12, ResourceId: 5}, Entry{Start: 0, End: 3, ResourceId: 3}, Entry{Start: 8, End: 15, ResourceId: 5}})
		expected := Report{ConflictPairs: 1, AffectedBookings: 2, LongestOverlap: 4}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 10, End: 12, ResourceId: 1}, Entry{Start: 10, End: 11, ResourceId: 4}, Entry{Start: 6, End: 9, ResourceId: 3}, Entry{Start: 10, End: 14, ResourceId: 1}, Entry{Start: 8, End: 9, ResourceId: 1}})
		expected := Report{ConflictPairs: 1, AffectedBookings: 2, LongestOverlap: 2}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 5, End: 9, ResourceId: 1}, Entry{Start: 3, End: 7, ResourceId: 3}, Entry{Start: 4, End: 10, ResourceId: 1}, Entry{Start: 9, End: 16, ResourceId: 5}, Entry{Start: 10, End: 12, ResourceId: 5}, Entry{Start: 9, End: 16, ResourceId: 1}})
		expected := Report{ConflictPairs: 3, AffectedBookings: 5, LongestOverlap: 4}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 8, ResourceId: 2}, Entry{Start: 3, End: 5, ResourceId: 3}, Entry{Start: 0, End: 6, ResourceId: 5}, Entry{Start: 2, End: 9, ResourceId: 4}, Entry{Start: 8, End: 13, ResourceId: 1}, Entry{Start: 8, End: 10, ResourceId: 1}, Entry{Start: 10, End: 11, ResourceId: 1}})
		expected := Report{ConflictPairs: 2, AffectedBookings: 3, LongestOverlap: 2}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 7, End: 14, ResourceId: 3}, Entry{Start: 8, End: 15, ResourceId: 1}, Entry{Start: 2, End: 9, ResourceId: 2}, Entry{Start: 10, End: 11, ResourceId: 3}, Entry{Start: 3, End: 5, ResourceId: 1}, Entry{Start: 5, End: 7, ResourceId: 2}, Entry{Start: 10, End: 12, ResourceId: 3}, Entry{Start: 4, End: 9, ResourceId: 4}})
		expected := Report{ConflictPairs: 4, AffectedBookings: 5, LongestOverlap: 2}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 2, ResourceId: 1}, Entry{Start: 2, End: 4, ResourceId: 1}})
		expected := Report{ConflictPairs: 0, AffectedBookings: 0, LongestOverlap: 0}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 3, ResourceId: 1}, Entry{Start: 0, End: 3, ResourceId: 1}, Entry{Start: 0, End: 3, ResourceId: 2}})
		expected := Report{ConflictPairs: 1, AffectedBookings: 2, LongestOverlap: 3}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 4, ResourceId: 1}, Entry{Start: 4, End: 9, ResourceId: 1}, Entry{Start: 2, End: 6, ResourceId: 1}, Entry{Start: 0, End: 8, ResourceId: 2}})
		expected := Report{ConflictPairs: 2, AffectedBookings: 3, LongestOverlap: 2}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
}
