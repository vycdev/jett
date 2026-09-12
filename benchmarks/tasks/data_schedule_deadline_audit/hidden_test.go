package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 5}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 1}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 8, Deadline: 23, Penalty: 3}}, 2)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 10}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 4, Deadline: 18, Penalty: 3}, Entry{Duration: 7, Deadline: 20, Penalty: 3}}, 3)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 14}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 1, Deadline: 0, Penalty: 2}, Entry{Duration: 2, Deadline: 2, Penalty: 4}, Entry{Duration: 4, Deadline: 11, Penalty: 3}}, 4)
		expected := Report{Late: 2, WeightedTardiness: 30, Finish: 11}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 6, Deadline: 18, Penalty: 2}, Entry{Duration: 8, Deadline: 2, Penalty: 2}, Entry{Duration: 7, Deadline: 27, Penalty: 3}, Entry{Duration: 1, Deadline: 17, Penalty: 3}}, 5)
		expected := Report{Late: 2, WeightedTardiness: 64, Finish: 27}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 3, Deadline: 12, Penalty: 1}, Entry{Duration: 4, Deadline: 8, Penalty: 1}, Entry{Duration: 7, Deadline: 16, Penalty: 2}, Entry{Duration: 2, Deadline: 24, Penalty: 4}, Entry{Duration: 6, Deadline: 14, Penalty: 1}}, 6)
		expected := Report{Late: 3, WeightedTardiness: 27, Finish: 28}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 7, Deadline: 19, Penalty: 2}, Entry{Duration: 1, Deadline: 7, Penalty: 3}, Entry{Duration: 2, Deadline: 14, Penalty: 2}, Entry{Duration: 1, Deadline: 0, Penalty: 4}, Entry{Duration: 7, Deadline: 21, Penalty: 4}, Entry{Duration: 1, Deadline: 17, Penalty: 4}}, 7)
		expected := Report{Late: 5, WeightedTardiness: 154, Finish: 26}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 3, Deadline: 29, Penalty: 3}, Entry{Duration: 1, Deadline: 1, Penalty: 3}, Entry{Duration: 4, Deadline: 21, Penalty: 4}, Entry{Duration: 3, Deadline: 21, Penalty: 1}, Entry{Duration: 8, Deadline: 3, Penalty: 3}, Entry{Duration: 3, Deadline: 26, Penalty: 4}, Entry{Duration: 2, Deadline: 15, Penalty: 3}}, 8)
		expected := Report{Late: 4, WeightedTardiness: 172, Finish: 32}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 7, Deadline: 24, Penalty: 2}, Entry{Duration: 2, Deadline: 24, Penalty: 3}, Entry{Duration: 4, Deadline: 21, Penalty: 3}, Entry{Duration: 3, Deadline: 27, Penalty: 3}, Entry{Duration: 3, Deadline: 13, Penalty: 3}, Entry{Duration: 3, Deadline: 8, Penalty: 3}, Entry{Duration: 6, Deadline: 15, Penalty: 2}, Entry{Duration: 1, Deadline: 10, Penalty: 1}}, 1)
		expected := Report{Late: 4, WeightedTardiness: 114, Finish: 30}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 8, Deadline: 0, Penalty: 4}, Entry{Duration: 6, Deadline: 6, Penalty: 1}, Entry{Duration: 2, Deadline: 15, Penalty: 1}, Entry{Duration: 1, Deadline: 4, Penalty: 1}, Entry{Duration: 2, Deadline: 20, Penalty: 2}, Entry{Duration: 6, Deadline: 12, Penalty: 3}, Entry{Duration: 2, Deadline: 10, Penalty: 3}, Entry{Duration: 5, Deadline: 29, Penalty: 3}, Entry{Duration: 3, Deadline: 6, Penalty: 1}}, 2)
		expected := Report{Late: 9, WeightedTardiness: 218, Finish: 37}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 5, Deadline: 6, Penalty: 3}, Entry{Duration: 5, Deadline: 16, Penalty: 4}, Entry{Duration: 7, Deadline: 29, Penalty: 1}, Entry{Duration: 4, Deadline: 22, Penalty: 4}, Entry{Duration: 8, Deadline: 6, Penalty: 1}, Entry{Duration: 5, Deadline: 29, Penalty: 1}, Entry{Duration: 7, Deadline: 7, Penalty: 2}, Entry{Duration: 3, Deadline: 24, Penalty: 2}, Entry{Duration: 4, Deadline: 13, Penalty: 2}, Entry{Duration: 3, Deadline: 21, Penalty: 2}}, 3)
		expected := Report{Late: 8, WeightedTardiness: 310, Finish: 54}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 8, Deadline: 14, Penalty: 3}, Entry{Duration: 6, Deadline: 8, Penalty: 3}, Entry{Duration: 4, Deadline: 6, Penalty: 1}, Entry{Duration: 6, Deadline: 4, Penalty: 2}, Entry{Duration: 5, Deadline: 5, Penalty: 2}, Entry{Duration: 2, Deadline: 4, Penalty: 1}, Entry{Duration: 8, Deadline: 28, Penalty: 4}, Entry{Duration: 1, Deadline: 12, Penalty: 4}, Entry{Duration: 3, Deadline: 16, Penalty: 1}, Entry{Duration: 8, Deadline: 2, Penalty: 4}, Entry{Duration: 7, Deadline: 21, Penalty: 3}}, 4)
		expected := Report{Late: 10, WeightedTardiness: 735, Finish: 62}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 2, Deadline: 5, Penalty: 4}, Entry{Duration: 1, Deadline: 13, Penalty: 4}, Entry{Duration: 3, Deadline: 29, Penalty: 3}, Entry{Duration: 1, Deadline: 17, Penalty: 2}, Entry{Duration: 8, Deadline: 14, Penalty: 3}, Entry{Duration: 1, Deadline: 15, Penalty: 2}, Entry{Duration: 4, Deadline: 9, Penalty: 3}, Entry{Duration: 7, Deadline: 16, Penalty: 1}, Entry{Duration: 5, Deadline: 9, Penalty: 2}, Entry{Duration: 1, Deadline: 21, Penalty: 1}, Entry{Duration: 8, Deadline: 13, Penalty: 3}, Entry{Duration: 5, Deadline: 25, Penalty: 4}}, 5)
		expected := Report{Late: 9, WeightedTardiness: 378, Finish: 51}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 6}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 1, Deadline: 16, Penalty: 1}}, 7)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 8}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 2, Deadline: 11, Penalty: 4}, Entry{Duration: 1, Deadline: 28, Penalty: 2}}, 8)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 11}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 8, Deadline: 23, Penalty: 3}, Entry{Duration: 5, Deadline: 21, Penalty: 1}, Entry{Duration: 3, Deadline: 19, Penalty: 1}}, 1)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 17}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 4, Deadline: 18, Penalty: 2}, Entry{Duration: 4, Deadline: 7, Penalty: 3}, Entry{Duration: 1, Deadline: 22, Penalty: 2}, Entry{Duration: 8, Deadline: 16, Penalty: 1}}, 2)
		expected := Report{Late: 2, WeightedTardiness: 12, Finish: 19}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 3, Deadline: 2, Penalty: 1}, Entry{Duration: 1, Deadline: 29, Penalty: 4}, Entry{Duration: 5, Deadline: 24, Penalty: 1}, Entry{Duration: 3, Deadline: 28, Penalty: 2}, Entry{Duration: 2, Deadline: 10, Penalty: 2}}, 3)
		expected := Report{Late: 2, WeightedTardiness: 18, Finish: 17}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 3, Deadline: 3, Penalty: 3}, Entry{Duration: 3, Deadline: 4, Penalty: 2}, Entry{Duration: 6, Deadline: 9, Penalty: 4}, Entry{Duration: 1, Deadline: 7, Penalty: 4}, Entry{Duration: 7, Deadline: 2, Penalty: 3}, Entry{Duration: 3, Deadline: 0, Penalty: 4}}, 4)
		expected := Report{Late: 6, WeightedTardiness: 266, Finish: 27}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 1, Deadline: 26, Penalty: 4}, Entry{Duration: 1, Deadline: 5, Penalty: 2}, Entry{Duration: 1, Deadline: 15, Penalty: 4}, Entry{Duration: 5, Deadline: 3, Penalty: 4}, Entry{Duration: 8, Deadline: 15, Penalty: 1}, Entry{Duration: 7, Deadline: 0, Penalty: 2}, Entry{Duration: 2, Deadline: 7, Penalty: 1}}, 5)
		expected := Report{Late: 5, WeightedTardiness: 129, Finish: 30}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 6, Deadline: 10, Penalty: 4}, Entry{Duration: 2, Deadline: 10, Penalty: 4}, Entry{Duration: 2, Deadline: 8, Penalty: 1}, Entry{Duration: 2, Deadline: 28, Penalty: 3}, Entry{Duration: 5, Deadline: 15, Penalty: 2}, Entry{Duration: 7, Deadline: 11, Penalty: 3}, Entry{Duration: 8, Deadline: 2, Penalty: 2}, Entry{Duration: 7, Deadline: 2, Penalty: 2}}, 6)
		expected := Report{Late: 7, WeightedTardiness: 263, Finish: 45}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 2, Deadline: 5, Penalty: 1}}, 3)
		expected := Report{Late: 0, WeightedTardiness: 0, Finish: 5}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 1, Deadline: 0, Penalty: 2}, Entry{Duration: 3, Deadline: 4, Penalty: 5}}, 0)
		expected := Report{Late: 1, WeightedTardiness: 2, Finish: 4}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Duration: 3, Deadline: 5, Penalty: 2}, Entry{Duration: 4, Deadline: 6, Penalty: 3}, Entry{Duration: 1, Deadline: 20, Penalty: 9}}, 2)
		expected := Report{Late: 1, WeightedTardiness: 9, Finish: 10}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
}
