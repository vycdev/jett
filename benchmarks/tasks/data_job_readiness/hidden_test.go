package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Ready: 0, Blocked: 0, TotalCost: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: 3}})
		expected := Report{Ready: 1, Blocked: 0, TotalCost: 3}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: 10}, Entry{Job: 2, Prerequisite: 1, Cost: 5}})
		expected := Report{Ready: 2, Blocked: 0, TotalCost: 15}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 3, Prerequisite: 0, Cost: 8}, Entry{Job: 1, Prerequisite: 0, Cost: (-8)}, Entry{Job: 2, Prerequisite: 0, Cost: (-6)}})
		expected := Report{Ready: 3, Blocked: 0, TotalCost: (-6)}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 3, Prerequisite: 0, Cost: 6}, Entry{Job: 1, Prerequisite: 0, Cost: 3}, Entry{Job: 2, Prerequisite: 1, Cost: 10}, Entry{Job: 4, Prerequisite: 0, Cost: (-2)}})
		expected := Report{Ready: 4, Blocked: 0, TotalCost: 17}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 5, Prerequisite: 4, Cost: (-1)}, Entry{Job: 2, Prerequisite: 1, Cost: (-7)}, Entry{Job: 3, Prerequisite: 0, Cost: 0}, Entry{Job: 4, Prerequisite: 0, Cost: 5}, Entry{Job: 1, Prerequisite: 0, Cost: (-4)}})
		expected := Report{Ready: 5, Blocked: 0, TotalCost: (-7)}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 5, Prerequisite: 2, Cost: (-5)}, Entry{Job: 3, Prerequisite: 2, Cost: (-2)}, Entry{Job: 2, Prerequisite: 1, Cost: 11}, Entry{Job: 4, Prerequisite: 0, Cost: (-1)}, Entry{Job: 6, Prerequisite: 3, Cost: (-4)}, Entry{Job: 1, Prerequisite: 0, Cost: (-6)}})
		expected := Report{Ready: 6, Blocked: 0, TotalCost: (-7)}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 6, Prerequisite: 1, Cost: (-7)}, Entry{Job: 7, Prerequisite: 6, Cost: 7}, Entry{Job: 4, Prerequisite: 0, Cost: 1}, Entry{Job: 2, Prerequisite: 1, Cost: (-3)}, Entry{Job: 5, Prerequisite: 1, Cost: 5}, Entry{Job: 3, Prerequisite: 1, Cost: (-8)}, Entry{Job: 1, Prerequisite: 0, Cost: 9}})
		expected := Report{Ready: 7, Blocked: 0, TotalCost: 4}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: 11}})
		expected := Report{Ready: 1, Blocked: 0, TotalCost: 11}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: 4}, Entry{Job: 2, Prerequisite: 0, Cost: (-5)}})
		expected := Report{Ready: 2, Blocked: 0, TotalCost: (-1)}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: 0}, Entry{Job: 3, Prerequisite: 1, Cost: (-3)}, Entry{Job: 2, Prerequisite: 0, Cost: 10}})
		expected := Report{Ready: 3, Blocked: 0, TotalCost: 7}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 4, Prerequisite: 1, Cost: (-7)}, Entry{Job: 2, Prerequisite: 1, Cost: 10}, Entry{Job: 1, Prerequisite: 0, Cost: 0}, Entry{Job: 3, Prerequisite: 1, Cost: 7}})
		expected := Report{Ready: 4, Blocked: 0, TotalCost: 10}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 2, Prerequisite: 1, Cost: (-2)}, Entry{Job: 3, Prerequisite: 0, Cost: (-5)}, Entry{Job: 4, Prerequisite: 3, Cost: (-8)}, Entry{Job: 1, Prerequisite: 0, Cost: 5}, Entry{Job: 5, Prerequisite: 0, Cost: (-4)}})
		expected := Report{Ready: 5, Blocked: 0, TotalCost: (-14)}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: 4}})
		expected := Report{Ready: 1, Blocked: 0, TotalCost: 4}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: (-5)}, Entry{Job: 2, Prerequisite: 1, Cost: 0}})
		expected := Report{Ready: 2, Blocked: 0, TotalCost: (-5)}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: (-3)}})
		expected := Report{Ready: 1, Blocked: 0, TotalCost: (-3)}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: (-5)}, Entry{Job: 2, Prerequisite: 1, Cost: (-2)}})
		expected := Report{Ready: 2, Blocked: 0, TotalCost: (-7)}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 1, Prerequisite: 0, Cost: 8}, Entry{Job: 2, Prerequisite: 1, Cost: 4}, Entry{Job: 3, Prerequisite: 0, Cost: (-2)}})
		expected := Report{Ready: 3, Blocked: 0, TotalCost: 10}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 4, Prerequisite: 3, Cost: (-1)}, Entry{Job: 1, Prerequisite: 0, Cost: (-2)}, Entry{Job: 3, Prerequisite: 0, Cost: 8}, Entry{Job: 2, Prerequisite: 0, Cost: 0}})
		expected := Report{Ready: 4, Blocked: 0, TotalCost: 5}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 4, Prerequisite: 3, Cost: 6}, Entry{Job: 1, Prerequisite: 0, Cost: (-2)}, Entry{Job: 2, Prerequisite: 1, Cost: (-2)}, Entry{Job: 5, Prerequisite: 2, Cost: 10}, Entry{Job: 3, Prerequisite: 0, Cost: (-3)}})
		expected := Report{Ready: 5, Blocked: 0, TotalCost: 9}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 3, Prerequisite: 0, Cost: (-1)}, Entry{Job: 2, Prerequisite: 0, Cost: 3}, Entry{Job: 6, Prerequisite: 0, Cost: (-4)}, Entry{Job: 5, Prerequisite: 1, Cost: 11}, Entry{Job: 4, Prerequisite: 2, Cost: (-3)}, Entry{Job: 1, Prerequisite: 0, Cost: 9}})
		expected := Report{Ready: 6, Blocked: 0, TotalCost: 15}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 9, Prerequisite: 8, Cost: 5}, Entry{Job: 3, Prerequisite: 2, Cost: 1}, Entry{Job: 2, Prerequisite: 1, Cost: 8}, Entry{Job: 1, Prerequisite: 0, Cost: (-2)}, Entry{Job: 10, Prerequisite: 9, Cost: 3}})
		expected := Report{Ready: 3, Blocked: 2, TotalCost: 7}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Job: 3, Prerequisite: 2, Cost: 5}, Entry{Job: 1, Prerequisite: 0, Cost: 2}, Entry{Job: 5, Prerequisite: 3, Cost: 9}})
		expected := Report{Ready: 1, Blocked: 2, TotalCost: 2}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
}
