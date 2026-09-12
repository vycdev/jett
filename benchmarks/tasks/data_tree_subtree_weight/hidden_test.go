package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 3}}, 2)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 10}, Entry{Node: 2, Parent: 1, Weight: 5}}, 3)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0, Weight: 8}, Entry{Node: 1, Parent: 0, Weight: (-8)}, Entry{Node: 2, Parent: 0, Weight: (-6)}}, 4)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0, Weight: 6}, Entry{Node: 1, Parent: 0, Weight: 3}, Entry{Node: 2, Parent: 1, Weight: 10}, Entry{Node: 4, Parent: 0, Weight: (-2)}}, 5)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 5, Parent: 4, Weight: (-1)}, Entry{Node: 2, Parent: 1, Weight: (-7)}, Entry{Node: 3, Parent: 0, Weight: 0}, Entry{Node: 4, Parent: 0, Weight: 5}, Entry{Node: 1, Parent: 0, Weight: (-4)}}, 6)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 5, Parent: 2, Weight: (-5)}, Entry{Node: 3, Parent: 2, Weight: (-2)}, Entry{Node: 2, Parent: 1, Weight: 11}, Entry{Node: 4, Parent: 0, Weight: (-1)}, Entry{Node: 6, Parent: 3, Weight: (-4)}, Entry{Node: 1, Parent: 0, Weight: (-6)}}, 7)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 6, Parent: 1, Weight: (-7)}, Entry{Node: 7, Parent: 6, Weight: 7}, Entry{Node: 4, Parent: 0, Weight: 1}, Entry{Node: 2, Parent: 1, Weight: (-3)}, Entry{Node: 5, Parent: 1, Weight: 5}, Entry{Node: 3, Parent: 1, Weight: (-8)}, Entry{Node: 1, Parent: 0, Weight: 9}}, 8)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 11}}, 1)
		expected := Report{NodeCount: 1, TotalWeight: 11, LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 4}, Entry{Node: 2, Parent: 0, Weight: (-5)}}, 2)
		expected := Report{NodeCount: 1, TotalWeight: (-5), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 0}, Entry{Node: 3, Parent: 1, Weight: (-3)}, Entry{Node: 2, Parent: 0, Weight: 10}}, 3)
		expected := Report{NodeCount: 1, TotalWeight: (-3), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 1, Weight: (-7)}, Entry{Node: 2, Parent: 1, Weight: 10}, Entry{Node: 1, Parent: 0, Weight: 0}, Entry{Node: 3, Parent: 1, Weight: 7}}, 4)
		expected := Report{NodeCount: 1, TotalWeight: (-7), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 2, Parent: 1, Weight: (-2)}, Entry{Node: 3, Parent: 0, Weight: (-5)}, Entry{Node: 4, Parent: 3, Weight: (-8)}, Entry{Node: 1, Parent: 0, Weight: 5}, Entry{Node: 5, Parent: 0, Weight: (-4)}}, 5)
		expected := Report{NodeCount: 1, TotalWeight: (-4), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 4}}, 7)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: (-5)}, Entry{Node: 2, Parent: 1, Weight: 0}}, 8)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: (-3)}}, 1)
		expected := Report{NodeCount: 1, TotalWeight: (-3), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: (-5)}, Entry{Node: 2, Parent: 1, Weight: (-2)}}, 2)
		expected := Report{NodeCount: 1, TotalWeight: (-2), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 8}, Entry{Node: 2, Parent: 1, Weight: 4}, Entry{Node: 3, Parent: 0, Weight: (-2)}}, 3)
		expected := Report{NodeCount: 1, TotalWeight: (-2), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 3, Weight: (-1)}, Entry{Node: 1, Parent: 0, Weight: (-2)}, Entry{Node: 3, Parent: 0, Weight: 8}, Entry{Node: 2, Parent: 0, Weight: 0}}, 4)
		expected := Report{NodeCount: 1, TotalWeight: (-1), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 3, Weight: 6}, Entry{Node: 1, Parent: 0, Weight: (-2)}, Entry{Node: 2, Parent: 1, Weight: (-2)}, Entry{Node: 5, Parent: 2, Weight: 10}, Entry{Node: 3, Parent: 0, Weight: (-3)}}, 5)
		expected := Report{NodeCount: 1, TotalWeight: 10, LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0, Weight: (-1)}, Entry{Node: 2, Parent: 0, Weight: 3}, Entry{Node: 6, Parent: 0, Weight: (-4)}, Entry{Node: 5, Parent: 1, Weight: 11}, Entry{Node: 4, Parent: 2, Weight: (-3)}, Entry{Node: 1, Parent: 0, Weight: 9}}, 6)
		expected := Report{NodeCount: 1, TotalWeight: (-4), LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 2}, Entry{Node: 2, Parent: 1, Weight: 3}, Entry{Node: 3, Parent: 1, Weight: 4}, Entry{Node: 4, Parent: 2, Weight: (-5)}, Entry{Node: 5, Parent: 0, Weight: 8}}, 1)
		expected := Report{NodeCount: 4, TotalWeight: 4, LeafCount: 2}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 2}}, 0)
		expected := Report{NodeCount: 0, TotalWeight: 0, LeafCount: 0}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 7}, Entry{Node: 2, Parent: 1, Weight: 5}, Entry{Node: 3, Parent: 1, Weight: 8}, Entry{Node: 4, Parent: 2, Weight: (-2)}}, 2)
		expected := Report{NodeCount: 2, TotalWeight: 3, LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 30, Parent: 2, Weight: 7}, Entry{Node: 12, Parent: 9, Weight: (-2)}, Entry{Node: 2, Parent: 0, Weight: 3}, Entry{Node: 9, Parent: 2, Weight: 5}}, 9)
		expected := Report{NodeCount: 2, TotalWeight: 3, LeafCount: 1}
		if actual != expected {
			t.Fatalf("fixture 27: got %+v expected %+v", actual, expected)
		}
	}
}
