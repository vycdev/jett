package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Selected: 0, RankSum: 0, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Selected: 0, RankSum: 0, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Selected: 0, RankSum: 0, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 8, Penalty: 3}}, 2)
		expected := Report{Selected: 1, RankSum: 1, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 6, Penalty: 3}, Entry{Score: 7, Penalty: 5}}, 3)
		expected := Report{Selected: 2, RankSum: 3, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 11, Penalty: 1}, Entry{Score: 0, Penalty: 6}, Entry{Score: (-2), Penalty: 5}}, 4)
		expected := Report{Selected: 3, RankSum: 6, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 10, Penalty: 2}, Entry{Score: 2, Penalty: 3}, Entry{Score: 0, Penalty: 4}, Entry{Score: 11, Penalty: 2}}, 5)
		expected := Report{Selected: 4, RankSum: 10, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 10, Penalty: 3}, Entry{Score: 5, Penalty: 5}, Entry{Score: 2, Penalty: 6}, Entry{Score: 3, Penalty: 1}, Entry{Score: 1, Penalty: 1}}, 6)
		expected := Report{Selected: 5, RankSum: 15, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 5, Penalty: 6}, Entry{Score: (-2), Penalty: 5}, Entry{Score: 11, Penalty: 6}, Entry{Score: 2, Penalty: 4}, Entry{Score: 10, Penalty: 6}, Entry{Score: 6, Penalty: 5}}, 7)
		expected := Report{Selected: 6, RankSum: 21, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 5, Penalty: 1}, Entry{Score: 1, Penalty: 1}, Entry{Score: (-1), Penalty: 1}, Entry{Score: 8, Penalty: 4}, Entry{Score: 7, Penalty: 4}, Entry{Score: 5, Penalty: 4}, Entry{Score: 11, Penalty: 3}}, 8)
		expected := Report{Selected: 7, RankSum: 28, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: (-3), Penalty: 3}, Entry{Score: 7, Penalty: 4}, Entry{Score: 7, Penalty: 1}, Entry{Score: (-2), Penalty: 3}, Entry{Score: (-1), Penalty: 4}, Entry{Score: 4, Penalty: 5}, Entry{Score: 8, Penalty: 4}, Entry{Score: (-2), Penalty: 6}}, 1)
		expected := Report{Selected: 1, RankSum: 1, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 10, Penalty: 2}, Entry{Score: 11, Penalty: 2}, Entry{Score: 2, Penalty: 2}, Entry{Score: 1, Penalty: 2}, Entry{Score: 1, Penalty: 5}, Entry{Score: 4, Penalty: 2}, Entry{Score: 2, Penalty: 1}, Entry{Score: (-3), Penalty: 4}, Entry{Score: 2, Penalty: 2}}, 2)
		expected := Report{Selected: 2, RankSum: 3, TieGroups: 1}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 7, Penalty: 1}, Entry{Score: (-3), Penalty: 1}, Entry{Score: 5, Penalty: 6}, Entry{Score: 10, Penalty: 1}, Entry{Score: 8, Penalty: 3}, Entry{Score: 6, Penalty: 3}, Entry{Score: 2, Penalty: 3}, Entry{Score: 5, Penalty: 3}, Entry{Score: (-1), Penalty: 2}, Entry{Score: 1, Penalty: 2}}, 3)
		expected := Report{Selected: 3, RankSum: 6, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 5, Penalty: 3}, Entry{Score: 3, Penalty: 6}, Entry{Score: 11, Penalty: 1}, Entry{Score: 8, Penalty: 6}, Entry{Score: 4, Penalty: 2}, Entry{Score: 1, Penalty: 1}, Entry{Score: 3, Penalty: 2}, Entry{Score: 5, Penalty: 6}, Entry{Score: 6, Penalty: 5}, Entry{Score: 9, Penalty: 5}, Entry{Score: 7, Penalty: 2}}, 4)
		expected := Report{Selected: 4, RankSum: 10, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 3, Penalty: 2}, Entry{Score: 7, Penalty: 2}, Entry{Score: 4, Penalty: 6}, Entry{Score: 6, Penalty: 3}, Entry{Score: 11, Penalty: 3}, Entry{Score: 0, Penalty: 5}, Entry{Score: 2, Penalty: 2}, Entry{Score: 10, Penalty: 3}, Entry{Score: (-1), Penalty: 5}, Entry{Score: (-1), Penalty: 1}, Entry{Score: 11, Penalty: 6}, Entry{Score: (-3), Penalty: 4}}, 5)
		expected := Report{Selected: 5, RankSum: 15, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Selected: 0, RankSum: 0, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: (-1), Penalty: 5}}, 7)
		expected := Report{Selected: 1, RankSum: 1, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 4, Penalty: 1}, Entry{Score: 8, Penalty: 4}}, 8)
		expected := Report{Selected: 2, RankSum: 3, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 7, Penalty: 3}, Entry{Score: (-1), Penalty: 4}, Entry{Score: 3, Penalty: 4}}, 1)
		expected := Report{Selected: 1, RankSum: 1, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 11, Penalty: 3}, Entry{Score: 5, Penalty: 2}, Entry{Score: 4, Penalty: 3}, Entry{Score: 4, Penalty: 2}}, 2)
		expected := Report{Selected: 2, RankSum: 3, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 1, Penalty: 5}, Entry{Score: 5, Penalty: 4}, Entry{Score: (-3), Penalty: 3}, Entry{Score: 5, Penalty: 5}, Entry{Score: (-3), Penalty: 6}}, 3)
		expected := Report{Selected: 3, RankSum: 6, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 4, Penalty: 4}, Entry{Score: 1, Penalty: 6}, Entry{Score: 10, Penalty: 1}, Entry{Score: (-3), Penalty: 1}, Entry{Score: 4, Penalty: 1}, Entry{Score: 4, Penalty: 6}}, 4)
		expected := Report{Selected: 4, RankSum: 10, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 1, Penalty: 6}, Entry{Score: 6, Penalty: 5}, Entry{Score: 6, Penalty: 5}, Entry{Score: 0, Penalty: 5}, Entry{Score: 0, Penalty: 2}, Entry{Score: 11, Penalty: 1}, Entry{Score: (-1), Penalty: 4}}, 5)
		expected := Report{Selected: 5, RankSum: 14, TieGroups: 1}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 6, Penalty: 1}, Entry{Score: (-1), Penalty: 1}, Entry{Score: (-3), Penalty: 4}, Entry{Score: 9, Penalty: 5}, Entry{Score: (-1), Penalty: 2}, Entry{Score: 2, Penalty: 2}, Entry{Score: (-2), Penalty: 3}, Entry{Score: (-1), Penalty: 6}}, 6)
		expected := Report{Selected: 6, RankSum: 21, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 9, Penalty: 1}, Entry{Score: 9, Penalty: 1}, Entry{Score: 8, Penalty: 1}}, 2)
		expected := Report{Selected: 2, RankSum: 2, TieGroups: 1}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 0, Penalty: 2}, Entry{Score: 0, Penalty: 1}}, 1)
		expected := Report{Selected: 1, RankSum: 1, TieGroups: 0}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Score: 9, Penalty: 1}, Entry{Score: 9, Penalty: 1}, Entry{Score: 9, Penalty: 2}, Entry{Score: 8, Penalty: 0}}, 3)
		expected := Report{Selected: 3, RankSum: 5, TieGroups: 1}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
}
