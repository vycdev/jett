package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}}, 2)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 1}}, 3)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0}, Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 0}}, 4)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0}, Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 1}, Entry{Node: 4, Parent: 0}}, 5)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 5, Parent: 4}, Entry{Node: 2, Parent: 1}, Entry{Node: 3, Parent: 0}, Entry{Node: 4, Parent: 0}, Entry{Node: 1, Parent: 0}}, 6)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 5, Parent: 2}, Entry{Node: 3, Parent: 2}, Entry{Node: 2, Parent: 1}, Entry{Node: 4, Parent: 0}, Entry{Node: 6, Parent: 3}, Entry{Node: 1, Parent: 0}}, 7)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 6, Parent: 1}, Entry{Node: 7, Parent: 6}, Entry{Node: 4, Parent: 0}, Entry{Node: 2, Parent: 1}, Entry{Node: 5, Parent: 1}, Entry{Node: 3, Parent: 1}, Entry{Node: 1, Parent: 0}}, 8)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}}, 1)
		expected := Report{Ancestor: 1, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 0}}, 2)
		expected := Report{Ancestor: 2, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 3, Parent: 1}, Entry{Node: 2, Parent: 0}}, 3)
		expected := Report{Ancestor: 3, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 1}, Entry{Node: 2, Parent: 1}, Entry{Node: 1, Parent: 0}, Entry{Node: 3, Parent: 1}}, 4)
		expected := Report{Ancestor: 4, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 2, Parent: 1}, Entry{Node: 3, Parent: 0}, Entry{Node: 4, Parent: 3}, Entry{Node: 1, Parent: 0}, Entry{Node: 5, Parent: 0}}, 5)
		expected := Report{Ancestor: 5, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}}, 7)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 1}}, 8)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 1}}, 2)
		expected := Report{Ancestor: 2, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 1}, Entry{Node: 3, Parent: 0}}, 3)
		expected := Report{Ancestor: 3, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 3}, Entry{Node: 1, Parent: 0}, Entry{Node: 3, Parent: 0}, Entry{Node: 2, Parent: 0}}, 4)
		expected := Report{Ancestor: 4, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 3}, Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 1}, Entry{Node: 5, Parent: 2}, Entry{Node: 3, Parent: 0}}, 5)
		expected := Report{Ancestor: 5, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0}, Entry{Node: 2, Parent: 0}, Entry{Node: 6, Parent: 0}, Entry{Node: 5, Parent: 1}, Entry{Node: 4, Parent: 2}, Entry{Node: 1, Parent: 0}}, 6)
		expected := Report{Ancestor: 6, SelectedDistance: 0, LargestDistance: 0}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 1}, Entry{Node: 3, Parent: 1}, Entry{Node: 4, Parent: 2}}, 1)
		expected := Report{Ancestor: 1, SelectedDistance: 0, LargestDistance: 2}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 0}}, 1)
		expected := Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0}, Entry{Node: 2, Parent: 1}, Entry{Node: 3, Parent: 1}, Entry{Node: 4, Parent: 2}}, 3)
		expected := Report{Ancestor: 1, SelectedDistance: 1, LargestDistance: 2}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 30, Parent: 2}, Entry{Node: 12, Parent: 9}, Entry{Node: 2, Parent: 0}, Entry{Node: 9, Parent: 2}}, 12)
		expected := Report{Ancestor: 2, SelectedDistance: 2, LargestDistance: 1}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
}
