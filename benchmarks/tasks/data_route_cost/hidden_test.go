package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Reachable: 0, CostSum: 0, MostExpensive: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Reachable: 1, CostSum: 0, MostExpensive: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Reachable: 1, CostSum: 0, MostExpensive: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1, Cost: 2}}, 2)
		expected := Report{Reachable: 2, CostSum: 2, MostExpensive: 2}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 2, Cost: 6}, Entry{Source: 1, Target: 2, Cost: 1}}, 3)
		expected := Report{Reachable: 1, CostSum: 0, MostExpensive: 0}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 3, Cost: 1}, Entry{Source: 0, Target: 3, Cost: 4}, Entry{Source: 0, Target: 2, Cost: 3}}, 4)
		expected := Report{Reachable: 3, CostSum: 4, MostExpensive: 3}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 3, Cost: 4}, Entry{Source: 0, Target: 2, Cost: 4}, Entry{Source: 2, Target: 3, Cost: 5}, Entry{Source: 2, Target: 3, Cost: 4}}, 5)
		expected := Report{Reachable: 3, CostSum: 12, MostExpensive: 8}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 2, Cost: 3}, Entry{Source: 0, Target: 4, Cost: 5}, Entry{Source: 1, Target: 2, Cost: 7}, Entry{Source: 4, Target: 5, Cost: 6}, Entry{Source: 4, Target: 5, Cost: 4}}, 6)
		expected := Report{Reachable: 4, CostSum: 17, MostExpensive: 9}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 6, Cost: 4}, Entry{Source: 4, Target: 5, Cost: 5}, Entry{Source: 0, Target: 2, Cost: 3}, Entry{Source: 0, Target: 4, Cost: 2}, Entry{Source: 0, Target: 1, Cost: 6}, Entry{Source: 3, Target: 5, Cost: 6}}, 7)
		expected := Report{Reachable: 6, CostSum: 22, MostExpensive: 7}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 3, Target: 4, Cost: 5}, Entry{Source: 3, Target: 5, Cost: 3}, Entry{Source: 6, Target: 7, Cost: 1}, Entry{Source: 2, Target: 4, Cost: 6}, Entry{Source: 3, Target: 5, Cost: 6}, Entry{Source: 6, Target: 7, Cost: 7}, Entry{Source: 3, Target: 4, Cost: 7}}, 8)
		expected := Report{Reachable: 1, CostSum: 0, MostExpensive: 0}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1, Cost: 7}, Entry{Source: 0, Target: 1, Cost: 4}, Entry{Source: 0, Target: 1, Cost: 7}, Entry{Source: 0, Target: 1, Cost: 7}, Entry{Source: 0, Target: 1, Cost: 6}, Entry{Source: 0, Target: 1, Cost: 7}, Entry{Source: 0, Target: 1, Cost: 4}, Entry{Source: 0, Target: 1, Cost: 3}, Entry{Source: 0, Target: 1, Cost: 4}}, 2)
		expected := Report{Reachable: 2, CostSum: 3, MostExpensive: 3}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1, Cost: 3}, Entry{Source: 0, Target: 2, Cost: 1}, Entry{Source: 1, Target: 2, Cost: 2}, Entry{Source: 0, Target: 1, Cost: 4}, Entry{Source: 0, Target: 1, Cost: 2}, Entry{Source: 0, Target: 1, Cost: 6}, Entry{Source: 0, Target: 2, Cost: 4}, Entry{Source: 1, Target: 2, Cost: 3}, Entry{Source: 1, Target: 2, Cost: 3}, Entry{Source: 0, Target: 1, Cost: 1}}, 3)
		expected := Report{Reachable: 3, CostSum: 2, MostExpensive: 1}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 2, Cost: 3}, Entry{Source: 2, Target: 3, Cost: 5}, Entry{Source: 1, Target: 3, Cost: 1}, Entry{Source: 0, Target: 3, Cost: 6}, Entry{Source: 1, Target: 3, Cost: 2}, Entry{Source: 2, Target: 3, Cost: 3}, Entry{Source: 0, Target: 3, Cost: 4}, Entry{Source: 0, Target: 1, Cost: 5}, Entry{Source: 2, Target: 3, Cost: 7}, Entry{Source: 2, Target: 3, Cost: 2}, Entry{Source: 1, Target: 2, Cost: 7}}, 4)
		expected := Report{Reachable: 4, CostSum: 17, MostExpensive: 8}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 4, Cost: 2}, Entry{Source: 3, Target: 4, Cost: 6}, Entry{Source: 2, Target: 4, Cost: 3}, Entry{Source: 2, Target: 3, Cost: 2}, Entry{Source: 0, Target: 3, Cost: 2}, Entry{Source: 1, Target: 3, Cost: 2}, Entry{Source: 1, Target: 4, Cost: 1}, Entry{Source: 1, Target: 2, Cost: 7}, Entry{Source: 3, Target: 4, Cost: 1}, Entry{Source: 3, Target: 4, Cost: 2}, Entry{Source: 0, Target: 4, Cost: 1}, Entry{Source: 3, Target: 4, Cost: 6}}, 5)
		expected := Report{Reachable: 3, CostSum: 3, MostExpensive: 2}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Reachable: 1, CostSum: 0, MostExpensive: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 3, Cost: 2}}, 7)
		expected := Report{Reachable: 1, CostSum: 0, MostExpensive: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 3, Target: 4, Cost: 4}, Entry{Source: 3, Target: 5, Cost: 3}}, 8)
		expected := Report{Reachable: 1, CostSum: 0, MostExpensive: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1, Cost: 4}, Entry{Source: 0, Target: 1, Cost: 1}, Entry{Source: 0, Target: 1, Cost: 2}, Entry{Source: 0, Target: 1, Cost: 5}}, 2)
		expected := Report{Reachable: 2, CostSum: 1, MostExpensive: 1}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 2, Cost: 3}, Entry{Source: 1, Target: 2, Cost: 1}, Entry{Source: 0, Target: 2, Cost: 4}, Entry{Source: 1, Target: 2, Cost: 7}, Entry{Source: 1, Target: 2, Cost: 5}}, 3)
		expected := Report{Reachable: 2, CostSum: 4, MostExpensive: 4}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1, Cost: 3}, Entry{Source: 1, Target: 2, Cost: 2}, Entry{Source: 1, Target: 3, Cost: 3}, Entry{Source: 2, Target: 3, Cost: 5}, Entry{Source: 2, Target: 3, Cost: 5}, Entry{Source: 2, Target: 3, Cost: 2}}, 4)
		expected := Report{Reachable: 4, CostSum: 14, MostExpensive: 6}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 2, Cost: 2}, Entry{Source: 2, Target: 3, Cost: 6}, Entry{Source: 1, Target: 3, Cost: 5}, Entry{Source: 0, Target: 2, Cost: 1}, Entry{Source: 0, Target: 1, Cost: 4}, Entry{Source: 2, Target: 3, Cost: 2}, Entry{Source: 1, Target: 4, Cost: 1}}, 5)
		expected := Report{Reachable: 5, CostSum: 13, MostExpensive: 5}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 3, Cost: 2}, Entry{Source: 0, Target: 3, Cost: 2}, Entry{Source: 1, Target: 3, Cost: 3}, Entry{Source: 2, Target: 5, Cost: 4}, Entry{Source: 0, Target: 2, Cost: 5}, Entry{Source: 3, Target: 5, Cost: 1}, Entry{Source: 2, Target: 5, Cost: 2}, Entry{Source: 0, Target: 5, Cost: 5}}, 6)
		expected := Report{Reachable: 4, CostSum: 10, MostExpensive: 5}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1, Cost: 9}, Entry{Source: 0, Target: 1, Cost: 3}, Entry{Source: 1, Target: 3, Cost: 5}, Entry{Source: 2, Target: 3, Cost: 1}, Entry{Source: 0, Target: 3, Cost: 12}}, 5)
		expected := Report{Reachable: 3, CostSum: 11, MostExpensive: 8}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1, Cost: 8}, Entry{Source: 0, Target: 2, Cost: 2}, Entry{Source: 1, Target: 3, Cost: 1}, Entry{Source: 2, Target: 3, Cost: 3}}, 5)
		expected := Report{Reachable: 4, CostSum: 15, MostExpensive: 8}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
}
