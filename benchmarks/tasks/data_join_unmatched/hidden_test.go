package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Side: 8, Quantity: 3}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Side: 6, Quantity: 3}, Entry{Key: 3, Side: 7, Quantity: 5}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Side: 11, Quantity: 1}, Entry{Key: 0, Side: 0, Quantity: 6}, Entry{Key: 0, Side: (-2), Quantity: 5}})
		expected := Report{Matched: 0, LeftOnly: 6, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Side: 10, Quantity: 2}, Entry{Key: 2, Side: 2, Quantity: 3}, Entry{Key: 4, Side: 0, Quantity: 4}, Entry{Key: 0, Side: 11, Quantity: 2}})
		expected := Report{Matched: 0, LeftOnly: 4, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Side: 10, Quantity: 3}, Entry{Key: 0, Side: 5, Quantity: 5}, Entry{Key: 4, Side: 2, Quantity: 6}, Entry{Key: 1, Side: 3, Quantity: 1}, Entry{Key: 1, Side: 1, Quantity: 1}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 1}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Side: 5, Quantity: 6}, Entry{Key: 1, Side: (-2), Quantity: 5}, Entry{Key: 3, Side: 11, Quantity: 6}, Entry{Key: 4, Side: 2, Quantity: 4}, Entry{Key: 0, Side: 10, Quantity: 6}, Entry{Key: 3, Side: 6, Quantity: 5}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Side: 5, Quantity: 1}, Entry{Key: 1, Side: 1, Quantity: 1}, Entry{Key: 3, Side: (-1), Quantity: 1}, Entry{Key: 0, Side: 8, Quantity: 4}, Entry{Key: 3, Side: 7, Quantity: 4}, Entry{Key: 0, Side: 5, Quantity: 4}, Entry{Key: 1, Side: 11, Quantity: 3}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 1}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Side: (-3), Quantity: 3}, Entry{Key: 1, Side: 7, Quantity: 4}, Entry{Key: 1, Side: 7, Quantity: 1}, Entry{Key: 3, Side: (-2), Quantity: 3}, Entry{Key: 4, Side: (-1), Quantity: 4}, Entry{Key: 0, Side: 4, Quantity: 5}, Entry{Key: 2, Side: 8, Quantity: 4}, Entry{Key: 1, Side: (-2), Quantity: 6}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Side: 10, Quantity: 2}, Entry{Key: 2, Side: 11, Quantity: 2}, Entry{Key: 4, Side: 2, Quantity: 2}, Entry{Key: 3, Side: 1, Quantity: 2}, Entry{Key: 2, Side: 1, Quantity: 5}, Entry{Key: 2, Side: 4, Quantity: 2}, Entry{Key: 0, Side: 2, Quantity: 1}, Entry{Key: 3, Side: (-3), Quantity: 4}, Entry{Key: 4, Side: 2, Quantity: 2}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 7}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Side: 7, Quantity: 1}, Entry{Key: 3, Side: (-3), Quantity: 1}, Entry{Key: 1, Side: 5, Quantity: 6}, Entry{Key: 0, Side: 10, Quantity: 1}, Entry{Key: 1, Side: 8, Quantity: 3}, Entry{Key: 3, Side: 6, Quantity: 3}, Entry{Key: 0, Side: 2, Quantity: 3}, Entry{Key: 4, Side: 5, Quantity: 3}, Entry{Key: 2, Side: (-1), Quantity: 2}, Entry{Key: 0, Side: 1, Quantity: 2}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 2}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Side: 5, Quantity: 3}, Entry{Key: 4, Side: 3, Quantity: 6}, Entry{Key: 3, Side: 11, Quantity: 1}, Entry{Key: 1, Side: 8, Quantity: 6}, Entry{Key: 3, Side: 4, Quantity: 2}, Entry{Key: 0, Side: 1, Quantity: 1}, Entry{Key: 4, Side: 3, Quantity: 2}, Entry{Key: 1, Side: 5, Quantity: 6}, Entry{Key: 4, Side: 6, Quantity: 5}, Entry{Key: 1, Side: 9, Quantity: 5}, Entry{Key: 4, Side: 7, Quantity: 2}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 1}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Side: 3, Quantity: 2}, Entry{Key: 1, Side: 7, Quantity: 2}, Entry{Key: 3, Side: 4, Quantity: 6}, Entry{Key: 2, Side: 6, Quantity: 3}, Entry{Key: 2, Side: 11, Quantity: 3}, Entry{Key: 1, Side: 0, Quantity: 5}, Entry{Key: 0, Side: 2, Quantity: 2}, Entry{Key: 1, Side: 10, Quantity: 3}, Entry{Key: 1, Side: (-1), Quantity: 5}, Entry{Key: 0, Side: (-1), Quantity: 1}, Entry{Key: 3, Side: 11, Quantity: 6}, Entry{Key: 3, Side: (-3), Quantity: 4}})
		expected := Report{Matched: 0, LeftOnly: 5, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Side: (-1), Quantity: 5}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Side: 4, Quantity: 1}, Entry{Key: 4, Side: 8, Quantity: 4}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Side: 7, Quantity: 3}, Entry{Key: 0, Side: (-1), Quantity: 4}, Entry{Key: 0, Side: 3, Quantity: 4}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Side: 11, Quantity: 3}, Entry{Key: 0, Side: 5, Quantity: 2}, Entry{Key: 3, Side: 4, Quantity: 3}, Entry{Key: 0, Side: 4, Quantity: 2}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Side: 1, Quantity: 5}, Entry{Key: 2, Side: 5, Quantity: 4}, Entry{Key: 4, Side: (-3), Quantity: 3}, Entry{Key: 2, Side: 5, Quantity: 5}, Entry{Key: 1, Side: (-3), Quantity: 6}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 5}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Side: 4, Quantity: 4}, Entry{Key: 2, Side: 1, Quantity: 6}, Entry{Key: 3, Side: 10, Quantity: 1}, Entry{Key: 4, Side: (-3), Quantity: 1}, Entry{Key: 2, Side: 4, Quantity: 1}, Entry{Key: 1, Side: 4, Quantity: 6}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 6}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Side: 1, Quantity: 6}, Entry{Key: 0, Side: 6, Quantity: 5}, Entry{Key: 1, Side: 6, Quantity: 5}, Entry{Key: 0, Side: 0, Quantity: 5}, Entry{Key: 1, Side: 0, Quantity: 2}, Entry{Key: 2, Side: 11, Quantity: 1}, Entry{Key: 4, Side: (-1), Quantity: 4}})
		expected := Report{Matched: 0, LeftOnly: 7, RightOnly: 6}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 4, Side: 6, Quantity: 1}, Entry{Key: 4, Side: (-1), Quantity: 1}, Entry{Key: 0, Side: (-3), Quantity: 4}, Entry{Key: 2, Side: 9, Quantity: 5}, Entry{Key: 0, Side: (-1), Quantity: 2}, Entry{Key: 0, Side: 2, Quantity: 2}, Entry{Key: 1, Side: (-2), Quantity: 3}, Entry{Key: 1, Side: (-1), Quantity: 6}})
		expected := Report{Matched: 0, LeftOnly: 0, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Side: 0, Quantity: 2}, Entry{Key: 0, Side: 0, Quantity: 3}, Entry{Key: 0, Side: 1, Quantity: 4}, Entry{Key: 1, Side: 2, Quantity: 9}})
		expected := Report{Matched: 4, LeftOnly: 1, RightOnly: 0}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Side: 0, Quantity: 4}, Entry{Key: 1, Side: 1, Quantity: 2}, Entry{Key: 2, Side: 1, Quantity: 5}, Entry{Key: 1, Side: 0, Quantity: 1}})
		expected := Report{Matched: 2, LeftOnly: 3, RightOnly: 5}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
}
