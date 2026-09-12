package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{RestockUnits: 0, AtRisk: 0, WorstShortfall: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{RestockUnits: 0, AtRisk: 0, WorstShortfall: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{RestockUnits: 0, AtRisk: 0, WorstShortfall: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 8, DailyDemand: 3}}, 2)
		expected := Report{RestockUnits: 0, AtRisk: 0, WorstShortfall: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 6, DailyDemand: 3}, Entry{Stock: 7, DailyDemand: 5}}, 3)
		expected := Report{RestockUnits: 11, AtRisk: 2, WorstShortfall: 8}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 11, DailyDemand: 1}, Entry{Stock: 0, DailyDemand: 6}, Entry{Stock: (-2), DailyDemand: 5}}, 4)
		expected := Report{RestockUnits: 46, AtRisk: 2, WorstShortfall: 24}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 10, DailyDemand: 2}, Entry{Stock: 2, DailyDemand: 3}, Entry{Stock: 0, DailyDemand: 4}, Entry{Stock: 11, DailyDemand: 2}}, 5)
		expected := Report{RestockUnits: 33, AtRisk: 2, WorstShortfall: 20}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 10, DailyDemand: 3}, Entry{Stock: 5, DailyDemand: 5}, Entry{Stock: 2, DailyDemand: 6}, Entry{Stock: 3, DailyDemand: 1}, Entry{Stock: 1, DailyDemand: 1}}, 6)
		expected := Report{RestockUnits: 75, AtRisk: 5, WorstShortfall: 34}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 5, DailyDemand: 6}, Entry{Stock: (-2), DailyDemand: 5}, Entry{Stock: 11, DailyDemand: 6}, Entry{Stock: 2, DailyDemand: 4}, Entry{Stock: 10, DailyDemand: 6}, Entry{Stock: 6, DailyDemand: 5}}, 7)
		expected := Report{RestockUnits: 192, AtRisk: 6, WorstShortfall: 37}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 5, DailyDemand: 1}, Entry{Stock: 1, DailyDemand: 1}, Entry{Stock: (-1), DailyDemand: 1}, Entry{Stock: 8, DailyDemand: 4}, Entry{Stock: 7, DailyDemand: 4}, Entry{Stock: 5, DailyDemand: 4}, Entry{Stock: 11, DailyDemand: 3}}, 8)
		expected := Report{RestockUnits: 108, AtRisk: 7, WorstShortfall: 27}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: (-3), DailyDemand: 3}, Entry{Stock: 7, DailyDemand: 4}, Entry{Stock: 7, DailyDemand: 1}, Entry{Stock: (-2), DailyDemand: 3}, Entry{Stock: (-1), DailyDemand: 4}, Entry{Stock: 4, DailyDemand: 5}, Entry{Stock: 8, DailyDemand: 4}, Entry{Stock: (-2), DailyDemand: 6}}, 1)
		expected := Report{RestockUnits: 25, AtRisk: 5, WorstShortfall: 8}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 10, DailyDemand: 2}, Entry{Stock: 11, DailyDemand: 2}, Entry{Stock: 2, DailyDemand: 2}, Entry{Stock: 1, DailyDemand: 2}, Entry{Stock: 1, DailyDemand: 5}, Entry{Stock: 4, DailyDemand: 2}, Entry{Stock: 2, DailyDemand: 1}, Entry{Stock: (-3), DailyDemand: 4}, Entry{Stock: 2, DailyDemand: 2}}, 2)
		expected := Report{RestockUnits: 27, AtRisk: 5, WorstShortfall: 11}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 7, DailyDemand: 1}, Entry{Stock: (-3), DailyDemand: 1}, Entry{Stock: 5, DailyDemand: 6}, Entry{Stock: 10, DailyDemand: 1}, Entry{Stock: 8, DailyDemand: 3}, Entry{Stock: 6, DailyDemand: 3}, Entry{Stock: 2, DailyDemand: 3}, Entry{Stock: 5, DailyDemand: 3}, Entry{Stock: (-1), DailyDemand: 2}, Entry{Stock: 1, DailyDemand: 2}}, 3)
		expected := Report{RestockUnits: 46, AtRisk: 8, WorstShortfall: 13}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 5, DailyDemand: 3}, Entry{Stock: 3, DailyDemand: 6}, Entry{Stock: 11, DailyDemand: 1}, Entry{Stock: 8, DailyDemand: 6}, Entry{Stock: 4, DailyDemand: 2}, Entry{Stock: 1, DailyDemand: 1}, Entry{Stock: 3, DailyDemand: 2}, Entry{Stock: 5, DailyDemand: 6}, Entry{Stock: 6, DailyDemand: 5}, Entry{Stock: 9, DailyDemand: 5}, Entry{Stock: 7, DailyDemand: 2}}, 4)
		expected := Report{RestockUnits: 101, AtRisk: 10, WorstShortfall: 21}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 3, DailyDemand: 2}, Entry{Stock: 7, DailyDemand: 2}, Entry{Stock: 4, DailyDemand: 6}, Entry{Stock: 6, DailyDemand: 3}, Entry{Stock: 11, DailyDemand: 3}, Entry{Stock: 0, DailyDemand: 5}, Entry{Stock: 2, DailyDemand: 2}, Entry{Stock: 10, DailyDemand: 3}, Entry{Stock: (-1), DailyDemand: 5}, Entry{Stock: (-1), DailyDemand: 1}, Entry{Stock: 11, DailyDemand: 6}, Entry{Stock: (-3), DailyDemand: 4}}, 5)
		expected := Report{RestockUnits: 161, AtRisk: 12, WorstShortfall: 26}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{RestockUnits: 0, AtRisk: 0, WorstShortfall: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: (-1), DailyDemand: 5}}, 7)
		expected := Report{RestockUnits: 36, AtRisk: 1, WorstShortfall: 36}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 4, DailyDemand: 1}, Entry{Stock: 8, DailyDemand: 4}}, 8)
		expected := Report{RestockUnits: 28, AtRisk: 2, WorstShortfall: 24}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 7, DailyDemand: 3}, Entry{Stock: (-1), DailyDemand: 4}, Entry{Stock: 3, DailyDemand: 4}}, 1)
		expected := Report{RestockUnits: 6, AtRisk: 2, WorstShortfall: 5}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 11, DailyDemand: 3}, Entry{Stock: 5, DailyDemand: 2}, Entry{Stock: 4, DailyDemand: 3}, Entry{Stock: 4, DailyDemand: 2}}, 2)
		expected := Report{RestockUnits: 2, AtRisk: 1, WorstShortfall: 2}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 1, DailyDemand: 5}, Entry{Stock: 5, DailyDemand: 4}, Entry{Stock: (-3), DailyDemand: 3}, Entry{Stock: 5, DailyDemand: 5}, Entry{Stock: (-3), DailyDemand: 6}}, 3)
		expected := Report{RestockUnits: 64, AtRisk: 5, WorstShortfall: 21}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 4, DailyDemand: 4}, Entry{Stock: 1, DailyDemand: 6}, Entry{Stock: 10, DailyDemand: 1}, Entry{Stock: (-3), DailyDemand: 1}, Entry{Stock: 4, DailyDemand: 1}, Entry{Stock: 4, DailyDemand: 6}}, 4)
		expected := Report{RestockUnits: 62, AtRisk: 4, WorstShortfall: 23}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 1, DailyDemand: 6}, Entry{Stock: 6, DailyDemand: 5}, Entry{Stock: 6, DailyDemand: 5}, Entry{Stock: 0, DailyDemand: 5}, Entry{Stock: 0, DailyDemand: 2}, Entry{Stock: 11, DailyDemand: 1}, Entry{Stock: (-1), DailyDemand: 4}}, 5)
		expected := Report{RestockUnits: 123, AtRisk: 6, WorstShortfall: 29}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 6, DailyDemand: 1}, Entry{Stock: (-1), DailyDemand: 1}, Entry{Stock: (-3), DailyDemand: 4}, Entry{Stock: 9, DailyDemand: 5}, Entry{Stock: (-1), DailyDemand: 2}, Entry{Stock: 2, DailyDemand: 2}, Entry{Stock: (-2), DailyDemand: 3}, Entry{Stock: (-1), DailyDemand: 6}}, 6)
		expected := Report{RestockUnits: 135, AtRisk: 7, WorstShortfall: 37}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: (-5), DailyDemand: 3}, Entry{Stock: 0, DailyDemand: 3}}, 0)
		expected := Report{RestockUnits: 5, AtRisk: 1, WorstShortfall: 5}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 6, DailyDemand: 3}, Entry{Stock: 5, DailyDemand: 3}}, 2)
		expected := Report{RestockUnits: 1, AtRisk: 1, WorstShortfall: 1}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Stock: 5, DailyDemand: 3}, Entry{Stock: 8, DailyDemand: 2}, Entry{Stock: (-2), DailyDemand: 1}}, 2)
		expected := Report{RestockUnits: 5, AtRisk: 2, WorstShortfall: 4}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
}
