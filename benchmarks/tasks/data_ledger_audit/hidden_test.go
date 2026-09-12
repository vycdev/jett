package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Total: 0, LowestBalance: 0, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Total: 0, LowestBalance: 5, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Total: 0, LowestBalance: 1, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 3, Delta: 8}}, 2)
		expected := Report{Total: 10, LowestBalance: 10, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 1, Delta: 6}, Entry{Account: 3, Delta: 7}}, 3)
		expected := Report{Total: 19, LowestBalance: 9, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 2, Delta: 11}, Entry{Account: 0, Delta: 0}, Entry{Account: 0, Delta: (-2)}}, 4)
		expected := Report{Total: 17, LowestBalance: 2, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 3, Delta: 10}, Entry{Account: 2, Delta: 2}, Entry{Account: 4, Delta: 0}, Entry{Account: 0, Delta: 11}}, 5)
		expected := Report{Total: 43, LowestBalance: 5, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 3, Delta: 10}, Entry{Account: 0, Delta: 5}, Entry{Account: 4, Delta: 2}, Entry{Account: 1, Delta: 3}, Entry{Account: 1, Delta: 1}}, 6)
		expected := Report{Total: 45, LowestBalance: 8, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 3, Delta: 5}, Entry{Account: 1, Delta: (-2)}, Entry{Account: 3, Delta: 11}, Entry{Account: 4, Delta: 2}, Entry{Account: 0, Delta: 10}, Entry{Account: 3, Delta: 6}}, 7)
		expected := Report{Total: 60, LowestBalance: 5, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 1, Delta: 5}, Entry{Account: 1, Delta: 1}, Entry{Account: 3, Delta: (-1)}, Entry{Account: 0, Delta: 8}, Entry{Account: 3, Delta: 7}, Entry{Account: 0, Delta: 5}, Entry{Account: 1, Delta: 11}}, 8)
		expected := Report{Total: 60, LowestBalance: 7, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 0, Delta: (-3)}, Entry{Account: 1, Delta: 7}, Entry{Account: 1, Delta: 7}, Entry{Account: 3, Delta: (-2)}, Entry{Account: 4, Delta: (-1)}, Entry{Account: 0, Delta: 4}, Entry{Account: 2, Delta: 8}, Entry{Account: 1, Delta: (-2)}}, 1)
		expected := Report{Total: 23, LowestBalance: (-2), FirstOverdraw: 0}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 2, Delta: 10}, Entry{Account: 2, Delta: 11}, Entry{Account: 4, Delta: 2}, Entry{Account: 3, Delta: 1}, Entry{Account: 2, Delta: 1}, Entry{Account: 2, Delta: 4}, Entry{Account: 0, Delta: 2}, Entry{Account: 3, Delta: (-3)}, Entry{Account: 4, Delta: 2}}, 2)
		expected := Report{Total: 38, LowestBalance: 0, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 0, Delta: 7}, Entry{Account: 3, Delta: (-3)}, Entry{Account: 1, Delta: 5}, Entry{Account: 0, Delta: 10}, Entry{Account: 1, Delta: 8}, Entry{Account: 3, Delta: 6}, Entry{Account: 0, Delta: 2}, Entry{Account: 4, Delta: 5}, Entry{Account: 2, Delta: (-1)}, Entry{Account: 0, Delta: 1}}, 3)
		expected := Report{Total: 55, LowestBalance: 0, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 2, Delta: 5}, Entry{Account: 4, Delta: 3}, Entry{Account: 3, Delta: 11}, Entry{Account: 1, Delta: 8}, Entry{Account: 3, Delta: 4}, Entry{Account: 0, Delta: 1}, Entry{Account: 4, Delta: 3}, Entry{Account: 1, Delta: 5}, Entry{Account: 4, Delta: 6}, Entry{Account: 1, Delta: 9}, Entry{Account: 4, Delta: 7}}, 4)
		expected := Report{Total: 82, LowestBalance: 5, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 1, Delta: 3}, Entry{Account: 1, Delta: 7}, Entry{Account: 3, Delta: 4}, Entry{Account: 2, Delta: 6}, Entry{Account: 2, Delta: 11}, Entry{Account: 1, Delta: 0}, Entry{Account: 0, Delta: 2}, Entry{Account: 1, Delta: 10}, Entry{Account: 1, Delta: (-1)}, Entry{Account: 0, Delta: (-1)}, Entry{Account: 3, Delta: 11}, Entry{Account: 3, Delta: (-3)}}, 5)
		expected := Report{Total: 69, LowestBalance: 6, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Total: 0, LowestBalance: 6, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 3, Delta: (-1)}}, 7)
		expected := Report{Total: 6, LowestBalance: 6, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 0, Delta: 4}, Entry{Account: 4, Delta: 8}}, 8)
		expected := Report{Total: 28, LowestBalance: 12, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 3, Delta: 7}, Entry{Account: 0, Delta: (-1)}, Entry{Account: 0, Delta: 3}}, 1)
		expected := Report{Total: 11, LowestBalance: 0, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 1, Delta: 11}, Entry{Account: 0, Delta: 5}, Entry{Account: 3, Delta: 4}, Entry{Account: 0, Delta: 4}}, 2)
		expected := Report{Total: 30, LowestBalance: 6, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 1, Delta: 1}, Entry{Account: 2, Delta: 5}, Entry{Account: 4, Delta: (-3)}, Entry{Account: 2, Delta: 5}, Entry{Account: 1, Delta: (-3)}}, 3)
		expected := Report{Total: 14, LowestBalance: 0, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 0, Delta: 4}, Entry{Account: 2, Delta: 1}, Entry{Account: 3, Delta: 10}, Entry{Account: 4, Delta: (-3)}, Entry{Account: 2, Delta: 4}, Entry{Account: 1, Delta: 4}}, 4)
		expected := Report{Total: 40, LowestBalance: 1, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 2, Delta: 1}, Entry{Account: 0, Delta: 6}, Entry{Account: 1, Delta: 6}, Entry{Account: 0, Delta: 0}, Entry{Account: 1, Delta: 0}, Entry{Account: 2, Delta: 11}, Entry{Account: 4, Delta: (-1)}}, 5)
		expected := Report{Total: 43, LowestBalance: 4, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 4, Delta: 6}, Entry{Account: 4, Delta: (-1)}, Entry{Account: 0, Delta: (-3)}, Entry{Account: 2, Delta: 9}, Entry{Account: 0, Delta: (-1)}, Entry{Account: 0, Delta: 2}, Entry{Account: 1, Delta: (-2)}, Entry{Account: 1, Delta: (-1)}}, 6)
		expected := Report{Total: 33, LowestBalance: 2, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 0, Delta: 3}, Entry{Account: 1, Delta: 6}}, 5)
		expected := Report{Total: 19, LowestBalance: 8, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 0, Delta: (-5)}, Entry{Account: 0, Delta: 1}}, 5)
		expected := Report{Total: 1, LowestBalance: 0, FirstOverdraw: (-1)}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 0, Delta: (-6)}, Entry{Account: 1, Delta: (-8)}}, 5)
		expected := Report{Total: (-4), LowestBalance: (-3), FirstOverdraw: 0}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Account: 2, Delta: (-7)}, Entry{Account: 2, Delta: 9}, Entry{Account: 3, Delta: (-1)}}, 5)
		expected := Report{Total: 11, LowestBalance: (-2), FirstOverdraw: 0}
		if actual != expected {
			t.Fatalf("fixture 27: got %+v expected %+v", actual, expected)
		}
	}
}
