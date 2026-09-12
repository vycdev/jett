package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Settled: 0, Outstanding: 0, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 3, Amount: 8}})
		expected := Report{Settled: 0, Outstanding: 8, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 1, Amount: 6}, Entry{Invoice: 3, Amount: 7}})
		expected := Report{Settled: 0, Outstanding: 13, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 2, Amount: 11}, Entry{Invoice: 0, Amount: 0}, Entry{Invoice: 0, Amount: (-2)}})
		expected := Report{Settled: 0, Outstanding: 11, Credit: 2}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 3, Amount: 10}, Entry{Invoice: 2, Amount: 2}, Entry{Invoice: 4, Amount: 0}, Entry{Invoice: 0, Amount: 11}})
		expected := Report{Settled: 1, Outstanding: 23, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 3, Amount: 10}, Entry{Invoice: 0, Amount: 5}, Entry{Invoice: 4, Amount: 2}, Entry{Invoice: 1, Amount: 3}, Entry{Invoice: 1, Amount: 1}})
		expected := Report{Settled: 0, Outstanding: 21, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 3, Amount: 5}, Entry{Invoice: 1, Amount: (-2)}, Entry{Invoice: 3, Amount: 11}, Entry{Invoice: 4, Amount: 2}, Entry{Invoice: 0, Amount: 10}, Entry{Invoice: 3, Amount: 6}})
		expected := Report{Settled: 0, Outstanding: 34, Credit: 2}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 1, Amount: 5}, Entry{Invoice: 1, Amount: 1}, Entry{Invoice: 3, Amount: (-1)}, Entry{Invoice: 0, Amount: 8}, Entry{Invoice: 3, Amount: 7}, Entry{Invoice: 0, Amount: 5}, Entry{Invoice: 1, Amount: 11}})
		expected := Report{Settled: 0, Outstanding: 36, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 0, Amount: (-3)}, Entry{Invoice: 1, Amount: 7}, Entry{Invoice: 1, Amount: 7}, Entry{Invoice: 3, Amount: (-2)}, Entry{Invoice: 4, Amount: (-1)}, Entry{Invoice: 0, Amount: 4}, Entry{Invoice: 2, Amount: 8}, Entry{Invoice: 1, Amount: (-2)}})
		expected := Report{Settled: 0, Outstanding: 21, Credit: 3}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 2, Amount: 10}, Entry{Invoice: 2, Amount: 11}, Entry{Invoice: 4, Amount: 2}, Entry{Invoice: 3, Amount: 1}, Entry{Invoice: 2, Amount: 1}, Entry{Invoice: 2, Amount: 4}, Entry{Invoice: 0, Amount: 2}, Entry{Invoice: 3, Amount: (-3)}, Entry{Invoice: 4, Amount: 2}})
		expected := Report{Settled: 0, Outstanding: 32, Credit: 2}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 0, Amount: 7}, Entry{Invoice: 3, Amount: (-3)}, Entry{Invoice: 1, Amount: 5}, Entry{Invoice: 0, Amount: 10}, Entry{Invoice: 1, Amount: 8}, Entry{Invoice: 3, Amount: 6}, Entry{Invoice: 0, Amount: 2}, Entry{Invoice: 4, Amount: 5}, Entry{Invoice: 2, Amount: (-1)}, Entry{Invoice: 0, Amount: 1}})
		expected := Report{Settled: 0, Outstanding: 41, Credit: 1}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 2, Amount: 5}, Entry{Invoice: 4, Amount: 3}, Entry{Invoice: 3, Amount: 11}, Entry{Invoice: 1, Amount: 8}, Entry{Invoice: 3, Amount: 4}, Entry{Invoice: 0, Amount: 1}, Entry{Invoice: 4, Amount: 3}, Entry{Invoice: 1, Amount: 5}, Entry{Invoice: 4, Amount: 6}, Entry{Invoice: 1, Amount: 9}, Entry{Invoice: 4, Amount: 7}})
		expected := Report{Settled: 0, Outstanding: 62, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 1, Amount: 3}, Entry{Invoice: 1, Amount: 7}, Entry{Invoice: 3, Amount: 4}, Entry{Invoice: 2, Amount: 6}, Entry{Invoice: 2, Amount: 11}, Entry{Invoice: 1, Amount: 0}, Entry{Invoice: 0, Amount: 2}, Entry{Invoice: 1, Amount: 10}, Entry{Invoice: 1, Amount: (-1)}, Entry{Invoice: 0, Amount: (-1)}, Entry{Invoice: 3, Amount: 11}, Entry{Invoice: 3, Amount: (-3)}})
		expected := Report{Settled: 0, Outstanding: 49, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 3, Amount: (-1)}})
		expected := Report{Settled: 0, Outstanding: 0, Credit: 1}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 0, Amount: 4}, Entry{Invoice: 4, Amount: 8}})
		expected := Report{Settled: 0, Outstanding: 12, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 3, Amount: 7}, Entry{Invoice: 0, Amount: (-1)}, Entry{Invoice: 0, Amount: 3}})
		expected := Report{Settled: 0, Outstanding: 9, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 1, Amount: 11}, Entry{Invoice: 0, Amount: 5}, Entry{Invoice: 3, Amount: 4}, Entry{Invoice: 0, Amount: 4}})
		expected := Report{Settled: 0, Outstanding: 24, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 1, Amount: 1}, Entry{Invoice: 2, Amount: 5}, Entry{Invoice: 4, Amount: (-3)}, Entry{Invoice: 2, Amount: 5}, Entry{Invoice: 1, Amount: (-3)}})
		expected := Report{Settled: 0, Outstanding: 10, Credit: 5}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 0, Amount: 4}, Entry{Invoice: 2, Amount: 1}, Entry{Invoice: 3, Amount: 10}, Entry{Invoice: 4, Amount: (-3)}, Entry{Invoice: 2, Amount: 4}, Entry{Invoice: 1, Amount: 4}})
		expected := Report{Settled: 0, Outstanding: 23, Credit: 3}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 2, Amount: 1}, Entry{Invoice: 0, Amount: 6}, Entry{Invoice: 1, Amount: 6}, Entry{Invoice: 0, Amount: 0}, Entry{Invoice: 1, Amount: 0}, Entry{Invoice: 2, Amount: 11}, Entry{Invoice: 4, Amount: (-1)}})
		expected := Report{Settled: 0, Outstanding: 24, Credit: 1}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 4, Amount: 6}, Entry{Invoice: 4, Amount: (-1)}, Entry{Invoice: 0, Amount: (-3)}, Entry{Invoice: 2, Amount: 9}, Entry{Invoice: 0, Amount: (-1)}, Entry{Invoice: 0, Amount: 2}, Entry{Invoice: 1, Amount: (-2)}, Entry{Invoice: 1, Amount: (-1)}})
		expected := Report{Settled: 0, Outstanding: 14, Credit: 5}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 1, Amount: 0}})
		expected := Report{Settled: 1, Outstanding: 0, Credit: 0}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 1, Amount: (-3)}, Entry{Invoice: 1, Amount: 3}, Entry{Invoice: 2, Amount: (-1)}, Entry{Invoice: 3, Amount: 2}})
		expected := Report{Settled: 1, Outstanding: 2, Credit: 1}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Invoice: 1, Amount: 5}, Entry{Invoice: 2, Amount: (-4)}, Entry{Invoice: 1, Amount: (-5)}, Entry{Invoice: 3, Amount: 7}})
		expected := Report{Settled: 1, Outstanding: 7, Credit: 4}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
}
