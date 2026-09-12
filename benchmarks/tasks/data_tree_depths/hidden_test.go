package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Roots: 0, MaxDepth: 0, WeightedDepth: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 3}})
		expected := Report{Roots: 1, MaxDepth: 0, WeightedDepth: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 10}, Entry{Node: 2, Parent: 1, Weight: 5}})
		expected := Report{Roots: 1, MaxDepth: 1, WeightedDepth: 5}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0, Weight: 8}, Entry{Node: 1, Parent: 0, Weight: (-8)}, Entry{Node: 2, Parent: 0, Weight: (-6)}})
		expected := Report{Roots: 3, MaxDepth: 0, WeightedDepth: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0, Weight: 6}, Entry{Node: 1, Parent: 0, Weight: 3}, Entry{Node: 2, Parent: 1, Weight: 10}, Entry{Node: 4, Parent: 0, Weight: (-2)}})
		expected := Report{Roots: 3, MaxDepth: 1, WeightedDepth: 10}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 5, Parent: 4, Weight: (-1)}, Entry{Node: 2, Parent: 1, Weight: (-7)}, Entry{Node: 3, Parent: 0, Weight: 0}, Entry{Node: 4, Parent: 0, Weight: 5}, Entry{Node: 1, Parent: 0, Weight: (-4)}})
		expected := Report{Roots: 3, MaxDepth: 1, WeightedDepth: (-8)}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 5, Parent: 2, Weight: (-5)}, Entry{Node: 3, Parent: 2, Weight: (-2)}, Entry{Node: 2, Parent: 1, Weight: 11}, Entry{Node: 4, Parent: 0, Weight: (-1)}, Entry{Node: 6, Parent: 3, Weight: (-4)}, Entry{Node: 1, Parent: 0, Weight: (-6)}})
		expected := Report{Roots: 2, MaxDepth: 3, WeightedDepth: (-15)}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 6, Parent: 1, Weight: (-7)}, Entry{Node: 7, Parent: 6, Weight: 7}, Entry{Node: 4, Parent: 0, Weight: 1}, Entry{Node: 2, Parent: 1, Weight: (-3)}, Entry{Node: 5, Parent: 1, Weight: 5}, Entry{Node: 3, Parent: 1, Weight: (-8)}, Entry{Node: 1, Parent: 0, Weight: 9}})
		expected := Report{Roots: 2, MaxDepth: 2, WeightedDepth: 1}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 11}})
		expected := Report{Roots: 1, MaxDepth: 0, WeightedDepth: 0}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 4}, Entry{Node: 2, Parent: 0, Weight: (-5)}})
		expected := Report{Roots: 2, MaxDepth: 0, WeightedDepth: 0}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 0}, Entry{Node: 3, Parent: 1, Weight: (-3)}, Entry{Node: 2, Parent: 0, Weight: 10}})
		expected := Report{Roots: 2, MaxDepth: 1, WeightedDepth: (-3)}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 1, Weight: (-7)}, Entry{Node: 2, Parent: 1, Weight: 10}, Entry{Node: 1, Parent: 0, Weight: 0}, Entry{Node: 3, Parent: 1, Weight: 7}})
		expected := Report{Roots: 1, MaxDepth: 1, WeightedDepth: 10}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 2, Parent: 1, Weight: (-2)}, Entry{Node: 3, Parent: 0, Weight: (-5)}, Entry{Node: 4, Parent: 3, Weight: (-8)}, Entry{Node: 1, Parent: 0, Weight: 5}, Entry{Node: 5, Parent: 0, Weight: (-4)}})
		expected := Report{Roots: 3, MaxDepth: 1, WeightedDepth: (-10)}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 4}})
		expected := Report{Roots: 1, MaxDepth: 0, WeightedDepth: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: (-5)}, Entry{Node: 2, Parent: 1, Weight: 0}})
		expected := Report{Roots: 1, MaxDepth: 1, WeightedDepth: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: (-3)}})
		expected := Report{Roots: 1, MaxDepth: 0, WeightedDepth: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: (-5)}, Entry{Node: 2, Parent: 1, Weight: (-2)}})
		expected := Report{Roots: 1, MaxDepth: 1, WeightedDepth: (-2)}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 1, Parent: 0, Weight: 8}, Entry{Node: 2, Parent: 1, Weight: 4}, Entry{Node: 3, Parent: 0, Weight: (-2)}})
		expected := Report{Roots: 2, MaxDepth: 1, WeightedDepth: 4}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 3, Weight: (-1)}, Entry{Node: 1, Parent: 0, Weight: (-2)}, Entry{Node: 3, Parent: 0, Weight: 8}, Entry{Node: 2, Parent: 0, Weight: 0}})
		expected := Report{Roots: 3, MaxDepth: 1, WeightedDepth: (-1)}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 3, Weight: 6}, Entry{Node: 1, Parent: 0, Weight: (-2)}, Entry{Node: 2, Parent: 1, Weight: (-2)}, Entry{Node: 5, Parent: 2, Weight: 10}, Entry{Node: 3, Parent: 0, Weight: (-3)}})
		expected := Report{Roots: 2, MaxDepth: 2, WeightedDepth: 24}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 3, Parent: 0, Weight: (-1)}, Entry{Node: 2, Parent: 0, Weight: 3}, Entry{Node: 6, Parent: 0, Weight: (-4)}, Entry{Node: 5, Parent: 1, Weight: 11}, Entry{Node: 4, Parent: 2, Weight: (-3)}, Entry{Node: 1, Parent: 0, Weight: 9}})
		expected := Report{Roots: 4, MaxDepth: 1, WeightedDepth: 8}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 5, Parent: 4, Weight: (-1)}, Entry{Node: 4, Parent: 3, Weight: 2}, Entry{Node: 3, Parent: 2, Weight: 4}, Entry{Node: 2, Parent: 1, Weight: 8}, Entry{Node: 1, Parent: 0, Weight: 3}})
		expected := Report{Roots: 1, MaxDepth: 4, WeightedDepth: 18}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 4, Parent: 2, Weight: 3}, Entry{Node: 2, Parent: 1, Weight: 5}, Entry{Node: 1, Parent: 0, Weight: 9}, Entry{Node: 3, Parent: 0, Weight: 7}})
		expected := Report{Roots: 2, MaxDepth: 2, WeightedDepth: 11}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Node: 30, Parent: 2, Weight: 7}, Entry{Node: 12, Parent: 9, Weight: (-2)}, Entry{Node: 2, Parent: 0, Weight: 3}, Entry{Node: 9, Parent: 2, Weight: 5}})
		expected := Report{Roots: 1, MaxDepth: 2, WeightedDepth: 8}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
}
