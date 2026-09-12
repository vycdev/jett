package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Reachable: 0, Unreachable: 0, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Reachable: 1, Unreachable: 4, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Reachable: 1, Unreachable: 0, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 1}}, 2)
		expected := Report{Reachable: 1, Unreachable: 1, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 1}, Entry{Source: 2, Target: 2}}, 3)
		expected := Report{Reachable: 1, Unreachable: 2, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 0}, Entry{Source: 1, Target: 0}, Entry{Source: 3, Target: 1}}, 4)
		expected := Report{Reachable: 1, Unreachable: 3, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 2}, Entry{Source: 1, Target: 3}, Entry{Source: 1, Target: 3}, Entry{Source: 2, Target: 0}}, 5)
		expected := Report{Reachable: 1, Unreachable: 4, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 4, Target: 4}, Entry{Source: 5, Target: 1}, Entry{Source: 0, Target: 1}, Entry{Source: 0, Target: 3}, Entry{Source: 5, Target: 1}}, 6)
		expected := Report{Reachable: 3, Unreachable: 3, ReachableEdgeCount: 2}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 6, Target: 4}, Entry{Source: 3, Target: 5}, Entry{Source: 2, Target: 3}, Entry{Source: 6, Target: 5}, Entry{Source: 4, Target: 4}, Entry{Source: 4, Target: 0}}, 7)
		expected := Report{Reachable: 1, Unreachable: 6, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 4, Target: 1}, Entry{Source: 2, Target: 0}, Entry{Source: 6, Target: 6}, Entry{Source: 6, Target: 0}, Entry{Source: 7, Target: 2}, Entry{Source: 0, Target: 0}, Entry{Source: 3, Target: 6}}, 8)
		expected := Report{Reachable: 1, Unreachable: 7, ReachableEdgeCount: 1}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}}, 1)
		expected := Report{Reachable: 1, Unreachable: 0, ReachableEdgeCount: 8}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 0}, Entry{Source: 1, Target: 1}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 1}, Entry{Source: 1, Target: 1}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 1}}, 2)
		expected := Report{Reachable: 2, Unreachable: 0, ReachableEdgeCount: 9}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 1}, Entry{Source: 0, Target: 1}, Entry{Source: 2, Target: 2}, Entry{Source: 1, Target: 0}, Entry{Source: 0, Target: 1}, Entry{Source: 1, Target: 2}, Entry{Source: 2, Target: 1}, Entry{Source: 2, Target: 1}, Entry{Source: 0, Target: 2}, Entry{Source: 1, Target: 1}}, 3)
		expected := Report{Reachable: 3, Unreachable: 0, ReachableEdgeCount: 10}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 2}, Entry{Source: 3, Target: 1}, Entry{Source: 1, Target: 1}, Entry{Source: 1, Target: 1}, Entry{Source: 1, Target: 1}, Entry{Source: 1, Target: 3}, Entry{Source: 2, Target: 2}, Entry{Source: 2, Target: 1}, Entry{Source: 0, Target: 2}, Entry{Source: 1, Target: 2}, Entry{Source: 1, Target: 0}}, 4)
		expected := Report{Reachable: 4, Unreachable: 0, ReachableEdgeCount: 11}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 3}, Entry{Source: 3, Target: 0}, Entry{Source: 3, Target: 1}, Entry{Source: 0, Target: 3}, Entry{Source: 4, Target: 3}, Entry{Source: 3, Target: 2}, Entry{Source: 1, Target: 3}, Entry{Source: 3, Target: 3}, Entry{Source: 2, Target: 0}, Entry{Source: 1, Target: 3}, Entry{Source: 2, Target: 0}, Entry{Source: 1, Target: 1}}, 5)
		expected := Report{Reachable: 4, Unreachable: 1, ReachableEdgeCount: 11}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Reachable: 1, Unreachable: 5, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 4, Target: 6}}, 7)
		expected := Report{Reachable: 1, Unreachable: 6, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 5, Target: 6}, Entry{Source: 0, Target: 4}}, 8)
		expected := Report{Reachable: 2, Unreachable: 6, ReachableEdgeCount: 1}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 0, Target: 0}}, 1)
		expected := Report{Reachable: 1, Unreachable: 0, ReachableEdgeCount: 3}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 1, Target: 0}, Entry{Source: 1, Target: 1}}, 2)
		expected := Report{Reachable: 1, Unreachable: 1, ReachableEdgeCount: 1}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 0}, Entry{Source: 2, Target: 2}, Entry{Source: 2, Target: 2}, Entry{Source: 2, Target: 0}, Entry{Source: 2, Target: 0}}, 3)
		expected := Report{Reachable: 1, Unreachable: 2, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 2}, Entry{Source: 0, Target: 1}, Entry{Source: 3, Target: 0}, Entry{Source: 1, Target: 0}, Entry{Source: 0, Target: 0}, Entry{Source: 2, Target: 0}}, 4)
		expected := Report{Reachable: 3, Unreachable: 1, ReachableEdgeCount: 5}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 1, Target: 0}, Entry{Source: 1, Target: 1}, Entry{Source: 2, Target: 1}, Entry{Source: 1, Target: 2}, Entry{Source: 4, Target: 3}, Entry{Source: 1, Target: 4}, Entry{Source: 4, Target: 3}}, 5)
		expected := Report{Reachable: 1, Unreachable: 4, ReachableEdgeCount: 0}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 4}, Entry{Source: 0, Target: 4}, Entry{Source: 3, Target: 0}, Entry{Source: 3, Target: 0}, Entry{Source: 1, Target: 5}, Entry{Source: 0, Target: 3}, Entry{Source: 5, Target: 3}, Entry{Source: 0, Target: 3}}, 6)
		expected := Report{Reachable: 3, Unreachable: 3, ReachableEdgeCount: 5}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 2, Target: 3}, Entry{Source: 1, Target: 2}, Entry{Source: 0, Target: 1}, Entry{Source: 3, Target: 1}, Entry{Source: 0, Target: 1}}, 5)
		expected := Report{Reachable: 4, Unreachable: 1, ReachableEdgeCount: 5}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Source: 0, Target: 1}, Entry{Source: 1, Target: 0}, Entry{Source: 3, Target: 4}, Entry{Source: 1, Target: 1}}, 5)
		expected := Report{Reachable: 2, Unreachable: 3, ReachableEdgeCount: 3}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
}
