package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 5}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 1}}, 2)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 2}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 1}, Entry{Source: 2, Target: 2}}, 3)
		expected := Report{Sources: 0, Sinks: 1, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 0}, Entry{Source: 1, Target: 0}, Entry{Source: 3, Target: 1}}, 4)
		expected := Report{Sources: 2, Sinks: 1, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 2}, Entry{Source: 1, Target: 3}, Entry{Source: 1, Target: 3}, Entry{Source: 2, Target: 0}}, 5)
		expected := Report{Sources: 1, Sinks: 2, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 4, Target: 4}, Entry{Source: 5, Target: 1}, Entry{Source: 0, Target: 1}, Entry{Source: 0, Target: 3}, Entry{Source: 5, Target: 1}}, 6)
		expected := Report{Sources: 2, Sinks: 2, Balanced: 2}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 6, Target: 4}, Entry{Source: 3, Target: 5}, Entry{Source: 2, Target: 3}, Entry{Source: 6, Target: 5}, Entry{Source: 4, Target: 4}, Entry{Source: 4, Target: 0}}, 7)
		expected := Report{Sources: 2, Sinks: 2, Balanced: 3}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 4, Target: 1}, Entry{Source: 2, Target: 0}, Entry{Source: 6, Target: 6}, Entry{Source: 6, Target: 0}, Entry{Source: 7, Target: 2}, Entry{Source: 0, Target: 0}, Entry{Source: 3, Target: 6}}, 8)
		expected := Report{Sources: 3, Sinks: 1, Balanced: 3}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}}, 1)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 0}, Entry{Source: 1, Target: 1}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 1}, Entry{Source: 1, Target: 1}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 1}}, 2)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 0}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 1}, Entry{Source: 0, Target: 1}, Entry{Source: 2, Target: 2}, Entry{Source: 1, Target: 0}, Entry{Source: 0, Target: 1}, Entry{Source: 1, Target: 2}, Entry{Source: 2, Target: 1}, Entry{Source: 2, Target: 1}, Entry{Source: 0, Target: 2}, Entry{Source: 1, Target: 1}}, 3)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 0}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 2}, Entry{Source: 3, Target: 1}, Entry{Source: 1, Target: 1}, Entry{Source: 1, Target: 1}, Entry{Source: 1, Target: 1}, Entry{Source: 1, Target: 3}, Entry{Source: 2, Target: 2}, Entry{Source: 2, Target: 1}, Entry{Source: 0, Target: 2}, Entry{Source: 1, Target: 2}, Entry{Source: 1, Target: 0}}, 4)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 3}, Entry{Source: 3, Target: 0}, Entry{Source: 3, Target: 1}, Entry{Source: 0, Target: 3}, Entry{Source: 4, Target: 3}, Entry{Source: 3, Target: 2}, Entry{Source: 1, Target: 3}, Entry{Source: 3, Target: 3}, Entry{Source: 2, Target: 0}, Entry{Source: 1, Target: 3}, Entry{Source: 2, Target: 0}, Entry{Source: 1, Target: 1}}, 5)
		expected := Report{Sources: 1, Sinks: 0, Balanced: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 6}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 4, Target: 6}}, 7)
		expected := Report{Sources: 1, Sinks: 1, Balanced: 5}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 5, Target: 6}, Entry{Source: 0, Target: 4}}, 8)
		expected := Report{Sources: 2, Sinks: 2, Balanced: 4}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}}, 1)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 1, Target: 0}, Entry{Source: 1, Target: 1}}, 2)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 0}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 0}, Entry{Source: 2, Target: 2}, Entry{Source: 2, Target: 2}, Entry{Source: 2, Target: 0}, Entry{Source: 2, Target: 0}}, 3)
		expected := Report{Sources: 0, Sinks: 1, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 2}, Entry{Source: 0, Target: 1}, Entry{Source: 3, Target: 0}, Entry{Source: 1, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 2, Target: 0}}, 4)
		expected := Report{Sources: 1, Sinks: 0, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 0}, Entry{Source: 1, Target: 1}, Entry{Source: 2, Target: 1}, Entry{Source: 1, Target: 2}, Entry{Source: 4, Target: 3}, Entry{Source: 1, Target: 4}, Entry{Source: 4, Target: 3}}, 5)
		expected := Report{Sources: 0, Sinks: 2, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 4}, Entry{Source: 0, Target: 4}, Entry{Source: 3, Target: 0}, Entry{Source: 3, Target: 0}, Entry{Source: 1, Target: 5}, Entry{Source: 0, Target: 3}, Entry{Source: 5, Target: 3}, Entry{Source: 0, Target: 3}}, 6)
		expected := Report{Sources: 2, Sinks: 1, Balanced: 1}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1}, Entry{Source: 0, Target: 1}, Entry{Source: 1, Target: 0}, Entry{Source: 2, Target: 2}}, 4)
		expected := Report{Sources: 0, Sinks: 0, Balanced: 2}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1}, Entry{Source: 0, Target: 1}, Entry{Source: 1, Target: 2}, Entry{Source: 3, Target: 3}}, 5)
		expected := Report{Sources: 1, Sinks: 1, Balanced: 2}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
}
