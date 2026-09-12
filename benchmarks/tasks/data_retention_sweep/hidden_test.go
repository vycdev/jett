package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Modified: 8, Size: 3}}, 2)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 1}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Modified: 6, Size: 3}, Entry{Key: 3, Modified: 7, Size: 5}}, 3)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 2}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Modified: 11, Size: 1}, Entry{Key: 0, Modified: 0, Size: 6}, Entry{Key: 0, Modified: (-2), Size: 5}}, 4)
		expected := Report{Removed: 1, Reclaimed: 5, Retained: 2}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Modified: 10, Size: 2}, Entry{Key: 2, Modified: 2, Size: 3}, Entry{Key: 4, Modified: 0, Size: 4}, Entry{Key: 0, Modified: 11, Size: 2}}, 5)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 4}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Modified: 10, Size: 3}, Entry{Key: 0, Modified: 5, Size: 5}, Entry{Key: 4, Modified: 2, Size: 6}, Entry{Key: 1, Modified: 3, Size: 1}, Entry{Key: 1, Modified: 1, Size: 1}}, 6)
		expected := Report{Removed: 1, Reclaimed: 1, Retained: 4}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Modified: 5, Size: 6}, Entry{Key: 1, Modified: (-2), Size: 5}, Entry{Key: 3, Modified: 11, Size: 6}, Entry{Key: 4, Modified: 2, Size: 4}, Entry{Key: 0, Modified: 10, Size: 6}, Entry{Key: 3, Modified: 6, Size: 5}}, 7)
		expected := Report{Removed: 2, Reclaimed: 11, Retained: 4}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Modified: 5, Size: 1}, Entry{Key: 1, Modified: 1, Size: 1}, Entry{Key: 3, Modified: (-1), Size: 1}, Entry{Key: 0, Modified: 8, Size: 4}, Entry{Key: 3, Modified: 7, Size: 4}, Entry{Key: 0, Modified: 5, Size: 4}, Entry{Key: 1, Modified: 11, Size: 3}}, 8)
		expected := Report{Removed: 4, Reclaimed: 7, Retained: 3}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Modified: (-3), Size: 3}, Entry{Key: 1, Modified: 7, Size: 4}, Entry{Key: 1, Modified: 7, Size: 1}, Entry{Key: 3, Modified: (-2), Size: 3}, Entry{Key: 4, Modified: (-1), Size: 4}, Entry{Key: 0, Modified: 4, Size: 5}, Entry{Key: 2, Modified: 8, Size: 4}, Entry{Key: 1, Modified: (-2), Size: 6}}, 1)
		expected := Report{Removed: 2, Reclaimed: 9, Retained: 6}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Modified: 10, Size: 2}, Entry{Key: 2, Modified: 11, Size: 2}, Entry{Key: 4, Modified: 2, Size: 2}, Entry{Key: 3, Modified: 1, Size: 2}, Entry{Key: 2, Modified: 1, Size: 5}, Entry{Key: 2, Modified: 4, Size: 2}, Entry{Key: 0, Modified: 2, Size: 1}, Entry{Key: 3, Modified: (-3), Size: 4}, Entry{Key: 4, Modified: 2, Size: 2}}, 2)
		expected := Report{Removed: 2, Reclaimed: 9, Retained: 7}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Modified: 7, Size: 1}, Entry{Key: 3, Modified: (-3), Size: 1}, Entry{Key: 1, Modified: 5, Size: 6}, Entry{Key: 0, Modified: 10, Size: 1}, Entry{Key: 1, Modified: 8, Size: 3}, Entry{Key: 3, Modified: 6, Size: 3}, Entry{Key: 0, Modified: 2, Size: 3}, Entry{Key: 4, Modified: 5, Size: 3}, Entry{Key: 2, Modified: (-1), Size: 2}, Entry{Key: 0, Modified: 1, Size: 2}}, 3)
		expected := Report{Removed: 3, Reclaimed: 6, Retained: 7}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Modified: 5, Size: 3}, Entry{Key: 4, Modified: 3, Size: 6}, Entry{Key: 3, Modified: 11, Size: 1}, Entry{Key: 1, Modified: 8, Size: 6}, Entry{Key: 3, Modified: 4, Size: 2}, Entry{Key: 0, Modified: 1, Size: 1}, Entry{Key: 4, Modified: 3, Size: 2}, Entry{Key: 1, Modified: 5, Size: 6}, Entry{Key: 4, Modified: 6, Size: 5}, Entry{Key: 1, Modified: 9, Size: 5}, Entry{Key: 4, Modified: 7, Size: 2}}, 4)
		expected := Report{Removed: 2, Reclaimed: 8, Retained: 9}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Modified: 3, Size: 2}, Entry{Key: 1, Modified: 7, Size: 2}, Entry{Key: 3, Modified: 4, Size: 6}, Entry{Key: 2, Modified: 6, Size: 3}, Entry{Key: 2, Modified: 11, Size: 3}, Entry{Key: 1, Modified: 0, Size: 5}, Entry{Key: 0, Modified: 2, Size: 2}, Entry{Key: 1, Modified: 10, Size: 3}, Entry{Key: 1, Modified: (-1), Size: 5}, Entry{Key: 0, Modified: (-1), Size: 1}, Entry{Key: 3, Modified: 11, Size: 6}, Entry{Key: 3, Modified: (-3), Size: 4}}, 5)
		expected := Report{Removed: 6, Reclaimed: 23, Retained: 6}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Modified: (-1), Size: 5}}, 7)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 1}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Modified: 4, Size: 1}, Entry{Key: 4, Modified: 8, Size: 4}}, 8)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 2}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Modified: 7, Size: 3}, Entry{Key: 0, Modified: (-1), Size: 4}, Entry{Key: 0, Modified: 3, Size: 4}}, 1)
		expected := Report{Removed: 1, Reclaimed: 4, Retained: 2}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Modified: 11, Size: 3}, Entry{Key: 0, Modified: 5, Size: 2}, Entry{Key: 3, Modified: 4, Size: 3}, Entry{Key: 0, Modified: 4, Size: 2}}, 2)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 4}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Modified: 1, Size: 5}, Entry{Key: 2, Modified: 5, Size: 4}, Entry{Key: 4, Modified: (-3), Size: 3}, Entry{Key: 2, Modified: 5, Size: 5}, Entry{Key: 1, Modified: (-3), Size: 6}}, 3)
		expected := Report{Removed: 1, Reclaimed: 6, Retained: 4}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Modified: 4, Size: 4}, Entry{Key: 2, Modified: 1, Size: 6}, Entry{Key: 3, Modified: 10, Size: 1}, Entry{Key: 4, Modified: (-3), Size: 1}, Entry{Key: 2, Modified: 4, Size: 1}, Entry{Key: 1, Modified: 4, Size: 6}}, 4)
		expected := Report{Removed: 1, Reclaimed: 6, Retained: 5}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Modified: 1, Size: 6}, Entry{Key: 0, Modified: 6, Size: 5}, Entry{Key: 1, Modified: 6, Size: 5}, Entry{Key: 0, Modified: 0, Size: 5}, Entry{Key: 1, Modified: 0, Size: 2}, Entry{Key: 2, Modified: 11, Size: 1}, Entry{Key: 4, Modified: (-1), Size: 4}}, 5)
		expected := Report{Removed: 3, Reclaimed: 13, Retained: 4}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 4, Modified: 6, Size: 1}, Entry{Key: 4, Modified: (-1), Size: 1}, Entry{Key: 0, Modified: (-3), Size: 4}, Entry{Key: 2, Modified: 9, Size: 5}, Entry{Key: 0, Modified: (-1), Size: 2}, Entry{Key: 0, Modified: 2, Size: 2}, Entry{Key: 1, Modified: (-2), Size: 3}, Entry{Key: 1, Modified: (-1), Size: 6}}, 6)
		expected := Report{Removed: 4, Reclaimed: 10, Retained: 4}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Modified: 2, Size: 3}, Entry{Key: 0, Modified: 2, Size: 7}}, 3)
		expected := Report{Removed: 1, Reclaimed: 3, Retained: 1}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Modified: 3, Size: 5}, Entry{Key: 0, Modified: 4, Size: 2}, Entry{Key: 1, Modified: 1, Size: 9}}, 3)
		expected := Report{Removed: 0, Reclaimed: 0, Retained: 3}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Modified: 2, Size: 5}, Entry{Key: 1, Modified: 2, Size: 7}, Entry{Key: 2, Modified: 1, Size: 9}, Entry{Key: 1, Modified: 5, Size: 4}}, 4)
		expected := Report{Removed: 2, Reclaimed: 12, Retained: 2}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
}
