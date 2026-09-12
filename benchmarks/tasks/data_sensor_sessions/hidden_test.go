package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Completed: 0, Active: 0, Rejected: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 3, Operation: 2, Timestamp: 1}})
		expected := Report{Completed: 0, Active: 0, Rejected: 1}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 4, Operation: 2, Timestamp: 4}, Entry{Sensor: 4, Operation: 2, Timestamp: (-2)}})
		expected := Report{Completed: 0, Active: 0, Rejected: 2}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 0, Operation: 1, Timestamp: 8}, Entry{Sensor: 0, Operation: 0, Timestamp: 6}, Entry{Sensor: 3, Operation: 1, Timestamp: 3}})
		expected := Report{Completed: 0, Active: 1, Rejected: 2}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 2, Operation: 2, Timestamp: 7}, Entry{Sensor: 1, Operation: 3, Timestamp: (-1)}, Entry{Sensor: 1, Operation: 3, Timestamp: 3}, Entry{Sensor: 0, Operation: 2, Timestamp: 9}})
		expected := Report{Completed: 0, Active: 0, Rejected: 4}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 1, Operation: 3, Timestamp: (-2)}, Entry{Sensor: 1, Operation: 2, Timestamp: (-1)}, Entry{Sensor: 3, Operation: 1, Timestamp: (-1)}, Entry{Sensor: 4, Operation: 3, Timestamp: 9}, Entry{Sensor: 4, Operation: 2, Timestamp: 5}})
		expected := Report{Completed: 0, Active: 0, Rejected: 5}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 0, Operation: 3, Timestamp: 7}, Entry{Sensor: 4, Operation: 1, Timestamp: 6}, Entry{Sensor: 0, Operation: 1, Timestamp: 2}, Entry{Sensor: 0, Operation: 3, Timestamp: 0}, Entry{Sensor: 0, Operation: 0, Timestamp: 9}, Entry{Sensor: 3, Operation: 3, Timestamp: 8}})
		expected := Report{Completed: 0, Active: 1, Rejected: 5}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 3, Operation: 0, Timestamp: 6}, Entry{Sensor: 3, Operation: 1, Timestamp: 3}, Entry{Sensor: 0, Operation: 0, Timestamp: 2}, Entry{Sensor: 1, Operation: 3, Timestamp: 0}, Entry{Sensor: 0, Operation: 3, Timestamp: (-1)}, Entry{Sensor: 2, Operation: 1, Timestamp: 4}, Entry{Sensor: 0, Operation: 3, Timestamp: 7}})
		expected := Report{Completed: 0, Active: 2, Rejected: 5}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 2, Operation: 3, Timestamp: 0}, Entry{Sensor: 0, Operation: 2, Timestamp: 1}, Entry{Sensor: 2, Operation: 1, Timestamp: 7}, Entry{Sensor: 2, Operation: 1, Timestamp: 4}, Entry{Sensor: 2, Operation: 1, Timestamp: 2}, Entry{Sensor: 2, Operation: 2, Timestamp: 5}, Entry{Sensor: 1, Operation: 0, Timestamp: 3}, Entry{Sensor: 0, Operation: 3, Timestamp: (-2)}})
		expected := Report{Completed: 0, Active: 1, Rejected: 7}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 3, Operation: 2, Timestamp: 1}, Entry{Sensor: 0, Operation: 0, Timestamp: 5}, Entry{Sensor: 0, Operation: 0, Timestamp: 0}, Entry{Sensor: 4, Operation: 0, Timestamp: (-1)}, Entry{Sensor: 1, Operation: 2, Timestamp: 4}, Entry{Sensor: 4, Operation: 2, Timestamp: 9}, Entry{Sensor: 0, Operation: 2, Timestamp: 2}, Entry{Sensor: 4, Operation: 2, Timestamp: 3}, Entry{Sensor: 1, Operation: 1, Timestamp: (-1)}})
		expected := Report{Completed: 0, Active: 1, Rejected: 8}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 2, Operation: 1, Timestamp: 3}, Entry{Sensor: 4, Operation: 2, Timestamp: 6}, Entry{Sensor: 3, Operation: 3, Timestamp: (-2)}, Entry{Sensor: 1, Operation: 3, Timestamp: 5}, Entry{Sensor: 1, Operation: 0, Timestamp: 2}, Entry{Sensor: 0, Operation: 3, Timestamp: 1}, Entry{Sensor: 1, Operation: 1, Timestamp: 7}, Entry{Sensor: 4, Operation: 1, Timestamp: 1}, Entry{Sensor: 3, Operation: 1, Timestamp: 0}, Entry{Sensor: 1, Operation: 3, Timestamp: 5}})
		expected := Report{Completed: 1, Active: 0, Rejected: 8}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 2, Operation: 2, Timestamp: 2}, Entry{Sensor: 2, Operation: 1, Timestamp: 1}, Entry{Sensor: 4, Operation: 0, Timestamp: 3}, Entry{Sensor: 1, Operation: 1, Timestamp: 2}, Entry{Sensor: 1, Operation: 1, Timestamp: 7}, Entry{Sensor: 0, Operation: 1, Timestamp: (-1)}, Entry{Sensor: 3, Operation: 3, Timestamp: (-2)}, Entry{Sensor: 3, Operation: 3, Timestamp: 0}, Entry{Sensor: 4, Operation: 0, Timestamp: 5}, Entry{Sensor: 0, Operation: 3, Timestamp: 4}, Entry{Sensor: 2, Operation: 0, Timestamp: 0}})
		expected := Report{Completed: 0, Active: 2, Rejected: 9}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 3, Operation: 0, Timestamp: 4}, Entry{Sensor: 3, Operation: 1, Timestamp: 3}, Entry{Sensor: 0, Operation: 1, Timestamp: 5}, Entry{Sensor: 3, Operation: 2, Timestamp: (-2)}, Entry{Sensor: 3, Operation: 1, Timestamp: 1}, Entry{Sensor: 2, Operation: 2, Timestamp: 6}, Entry{Sensor: 3, Operation: 0, Timestamp: 2}, Entry{Sensor: 2, Operation: 1, Timestamp: (-2)}, Entry{Sensor: 0, Operation: 3, Timestamp: 4}, Entry{Sensor: 2, Operation: 2, Timestamp: 8}, Entry{Sensor: 3, Operation: 0, Timestamp: 6}, Entry{Sensor: 0, Operation: 0, Timestamp: 3}})
		expected := Report{Completed: 0, Active: 2, Rejected: 10}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 3, Operation: 0, Timestamp: 1}})
		expected := Report{Completed: 0, Active: 1, Rejected: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 3, Operation: 2, Timestamp: 2}, Entry{Sensor: 0, Operation: 1, Timestamp: 7}})
		expected := Report{Completed: 0, Active: 0, Rejected: 2}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 4, Operation: 0, Timestamp: 1}, Entry{Sensor: 4, Operation: 1, Timestamp: 1}, Entry{Sensor: 1, Operation: 2, Timestamp: (-2)}})
		expected := Report{Completed: 1, Active: 0, Rejected: 1}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 4, Operation: 1, Timestamp: 5}, Entry{Sensor: 4, Operation: 0, Timestamp: 6}, Entry{Sensor: 1, Operation: 0, Timestamp: 8}, Entry{Sensor: 0, Operation: 0, Timestamp: 5}})
		expected := Report{Completed: 0, Active: 3, Rejected: 1}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 2, Operation: 0, Timestamp: 0}, Entry{Sensor: 1, Operation: 0, Timestamp: 3}, Entry{Sensor: 1, Operation: 1, Timestamp: (-1)}, Entry{Sensor: 2, Operation: 1, Timestamp: 0}, Entry{Sensor: 1, Operation: 2, Timestamp: 2}})
		expected := Report{Completed: 1, Active: 1, Rejected: 2}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 4, Operation: 3, Timestamp: (-2)}, Entry{Sensor: 1, Operation: 3, Timestamp: 6}, Entry{Sensor: 3, Operation: 0, Timestamp: 2}, Entry{Sensor: 4, Operation: 1, Timestamp: (-2)}, Entry{Sensor: 4, Operation: 3, Timestamp: (-2)}, Entry{Sensor: 3, Operation: 0, Timestamp: 0}})
		expected := Report{Completed: 0, Active: 1, Rejected: 5}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 1, Operation: 0, Timestamp: 5}, Entry{Sensor: 3, Operation: 2, Timestamp: (-1)}, Entry{Sensor: 3, Operation: 3, Timestamp: 5}, Entry{Sensor: 0, Operation: 3, Timestamp: (-2)}, Entry{Sensor: 4, Operation: 1, Timestamp: (-1)}, Entry{Sensor: 1, Operation: 0, Timestamp: 3}, Entry{Sensor: 2, Operation: 3, Timestamp: (-1)}})
		expected := Report{Completed: 0, Active: 1, Rejected: 6}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 2, Operation: 3, Timestamp: (-1)}, Entry{Sensor: 2, Operation: 0, Timestamp: (-1)}, Entry{Sensor: 2, Operation: 2, Timestamp: 5}, Entry{Sensor: 1, Operation: 3, Timestamp: 3}, Entry{Sensor: 4, Operation: 2, Timestamp: 6}, Entry{Sensor: 3, Operation: 0, Timestamp: 1}, Entry{Sensor: 3, Operation: 0, Timestamp: 1}, Entry{Sensor: 3, Operation: 0, Timestamp: 9}})
		expected := Report{Completed: 0, Active: 1, Rejected: 7}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 1, Operation: 0, Timestamp: 0}, Entry{Sensor: 1, Operation: 1, Timestamp: 0}, Entry{Sensor: 1, Operation: 1, Timestamp: 1}})
		expected := Report{Completed: 1, Active: 0, Rejected: 1}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 1, Operation: 0, Timestamp: (-1)}, Entry{Sensor: 1, Operation: 0, Timestamp: 2}, Entry{Sensor: 1, Operation: 0, Timestamp: 3}, Entry{Sensor: 1, Operation: 1, Timestamp: 1}, Entry{Sensor: 1, Operation: 1, Timestamp: 2}})
		expected := Report{Completed: 1, Active: 0, Rejected: 3}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Sensor: 1, Operation: 0, Timestamp: 2}, Entry{Sensor: 1, Operation: 1, Timestamp: 1}, Entry{Sensor: 1, Operation: 1, Timestamp: 3}, Entry{Sensor: 2, Operation: 0, Timestamp: 0}})
		expected := Report{Completed: 1, Active: 1, Rejected: 1}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
}
