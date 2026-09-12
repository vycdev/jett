package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Accepted: 0, Rejected: 0, ExpiresSum: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 3, Timestamp: 8, Duration: 3}})
		expected := Report{Accepted: 1, Rejected: 0, ExpiresSum: 11}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 1, Timestamp: 6, Duration: 3}, Entry{ResourceId: 3, Timestamp: 7, Duration: 5}})
		expected := Report{Accepted: 2, Rejected: 0, ExpiresSum: 21}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 2, Timestamp: 11, Duration: 1}, Entry{ResourceId: 0, Timestamp: 0, Duration: 6}, Entry{ResourceId: 0, Timestamp: (-2), Duration: 5}})
		expected := Report{Accepted: 2, Rejected: 1, ExpiresSum: 18}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 3, Timestamp: 10, Duration: 2}, Entry{ResourceId: 2, Timestamp: 2, Duration: 3}, Entry{ResourceId: 4, Timestamp: 0, Duration: 4}, Entry{ResourceId: 0, Timestamp: 11, Duration: 2}})
		expected := Report{Accepted: 4, Rejected: 0, ExpiresSum: 34}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 3, Timestamp: 10, Duration: 3}, Entry{ResourceId: 0, Timestamp: 5, Duration: 5}, Entry{ResourceId: 4, Timestamp: 2, Duration: 6}, Entry{ResourceId: 1, Timestamp: 3, Duration: 1}, Entry{ResourceId: 1, Timestamp: 1, Duration: 1}})
		expected := Report{Accepted: 4, Rejected: 1, ExpiresSum: 35}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 3, Timestamp: 5, Duration: 6}, Entry{ResourceId: 1, Timestamp: (-2), Duration: 5}, Entry{ResourceId: 3, Timestamp: 11, Duration: 6}, Entry{ResourceId: 4, Timestamp: 2, Duration: 4}, Entry{ResourceId: 0, Timestamp: 10, Duration: 6}, Entry{ResourceId: 3, Timestamp: 6, Duration: 5}})
		expected := Report{Accepted: 4, Rejected: 2, ExpiresSum: 39}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 1, Timestamp: 5, Duration: 1}, Entry{ResourceId: 1, Timestamp: 1, Duration: 1}, Entry{ResourceId: 3, Timestamp: (-1), Duration: 1}, Entry{ResourceId: 0, Timestamp: 8, Duration: 4}, Entry{ResourceId: 3, Timestamp: 7, Duration: 4}, Entry{ResourceId: 0, Timestamp: 5, Duration: 4}, Entry{ResourceId: 1, Timestamp: 11, Duration: 3}})
		expected := Report{Accepted: 4, Rejected: 3, ExpiresSum: 37}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 0, Timestamp: (-3), Duration: 3}, Entry{ResourceId: 1, Timestamp: 7, Duration: 4}, Entry{ResourceId: 1, Timestamp: 7, Duration: 1}, Entry{ResourceId: 3, Timestamp: (-2), Duration: 3}, Entry{ResourceId: 4, Timestamp: (-1), Duration: 4}, Entry{ResourceId: 0, Timestamp: 4, Duration: 5}, Entry{ResourceId: 2, Timestamp: 8, Duration: 4}, Entry{ResourceId: 1, Timestamp: (-2), Duration: 6}})
		expected := Report{Accepted: 3, Rejected: 5, ExpiresSum: 32}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 2, Timestamp: 10, Duration: 2}, Entry{ResourceId: 2, Timestamp: 11, Duration: 2}, Entry{ResourceId: 4, Timestamp: 2, Duration: 2}, Entry{ResourceId: 3, Timestamp: 1, Duration: 2}, Entry{ResourceId: 2, Timestamp: 1, Duration: 5}, Entry{ResourceId: 2, Timestamp: 4, Duration: 2}, Entry{ResourceId: 0, Timestamp: 2, Duration: 1}, Entry{ResourceId: 3, Timestamp: (-3), Duration: 4}, Entry{ResourceId: 4, Timestamp: 2, Duration: 2}})
		expected := Report{Accepted: 4, Rejected: 5, ExpiresSum: 22}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 0, Timestamp: 7, Duration: 1}, Entry{ResourceId: 3, Timestamp: (-3), Duration: 1}, Entry{ResourceId: 1, Timestamp: 5, Duration: 6}, Entry{ResourceId: 0, Timestamp: 10, Duration: 1}, Entry{ResourceId: 1, Timestamp: 8, Duration: 3}, Entry{ResourceId: 3, Timestamp: 6, Duration: 3}, Entry{ResourceId: 0, Timestamp: 2, Duration: 3}, Entry{ResourceId: 4, Timestamp: 5, Duration: 3}, Entry{ResourceId: 2, Timestamp: (-1), Duration: 2}, Entry{ResourceId: 0, Timestamp: 1, Duration: 2}})
		expected := Report{Accepted: 5, Rejected: 5, ExpiresSum: 39}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 2, Timestamp: 5, Duration: 3}, Entry{ResourceId: 4, Timestamp: 3, Duration: 6}, Entry{ResourceId: 3, Timestamp: 11, Duration: 1}, Entry{ResourceId: 1, Timestamp: 8, Duration: 6}, Entry{ResourceId: 3, Timestamp: 4, Duration: 2}, Entry{ResourceId: 0, Timestamp: 1, Duration: 1}, Entry{ResourceId: 4, Timestamp: 3, Duration: 2}, Entry{ResourceId: 1, Timestamp: 5, Duration: 6}, Entry{ResourceId: 4, Timestamp: 6, Duration: 5}, Entry{ResourceId: 1, Timestamp: 9, Duration: 5}, Entry{ResourceId: 4, Timestamp: 7, Duration: 2}})
		expected := Report{Accepted: 5, Rejected: 6, ExpiresSum: 45}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 1, Timestamp: 3, Duration: 2}, Entry{ResourceId: 1, Timestamp: 7, Duration: 2}, Entry{ResourceId: 3, Timestamp: 4, Duration: 6}, Entry{ResourceId: 2, Timestamp: 6, Duration: 3}, Entry{ResourceId: 2, Timestamp: 11, Duration: 3}, Entry{ResourceId: 1, Timestamp: 0, Duration: 5}, Entry{ResourceId: 0, Timestamp: 2, Duration: 2}, Entry{ResourceId: 1, Timestamp: 10, Duration: 3}, Entry{ResourceId: 1, Timestamp: (-1), Duration: 5}, Entry{ResourceId: 0, Timestamp: (-1), Duration: 1}, Entry{ResourceId: 3, Timestamp: 11, Duration: 6}, Entry{ResourceId: 3, Timestamp: (-3), Duration: 4}})
		expected := Report{Accepted: 8, Rejected: 4, ExpiresSum: 48}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 3, Timestamp: (-1), Duration: 5}})
		expected := Report{Accepted: 0, Rejected: 1, ExpiresSum: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 0, Timestamp: 4, Duration: 1}, Entry{ResourceId: 4, Timestamp: 8, Duration: 4}})
		expected := Report{Accepted: 2, Rejected: 0, ExpiresSum: 17}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 3, Timestamp: 7, Duration: 3}, Entry{ResourceId: 0, Timestamp: (-1), Duration: 4}, Entry{ResourceId: 0, Timestamp: 3, Duration: 4}})
		expected := Report{Accepted: 2, Rejected: 1, ExpiresSum: 17}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 1, Timestamp: 11, Duration: 3}, Entry{ResourceId: 0, Timestamp: 5, Duration: 2}, Entry{ResourceId: 3, Timestamp: 4, Duration: 3}, Entry{ResourceId: 0, Timestamp: 4, Duration: 2}})
		expected := Report{Accepted: 3, Rejected: 1, ExpiresSum: 28}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 1, Timestamp: 1, Duration: 5}, Entry{ResourceId: 2, Timestamp: 5, Duration: 4}, Entry{ResourceId: 4, Timestamp: (-3), Duration: 3}, Entry{ResourceId: 2, Timestamp: 5, Duration: 5}, Entry{ResourceId: 1, Timestamp: (-3), Duration: 6}})
		expected := Report{Accepted: 2, Rejected: 3, ExpiresSum: 15}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 0, Timestamp: 4, Duration: 4}, Entry{ResourceId: 2, Timestamp: 1, Duration: 6}, Entry{ResourceId: 3, Timestamp: 10, Duration: 1}, Entry{ResourceId: 4, Timestamp: (-3), Duration: 1}, Entry{ResourceId: 2, Timestamp: 4, Duration: 1}, Entry{ResourceId: 1, Timestamp: 4, Duration: 6}})
		expected := Report{Accepted: 4, Rejected: 2, ExpiresSum: 36}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 2, Timestamp: 1, Duration: 6}, Entry{ResourceId: 0, Timestamp: 6, Duration: 5}, Entry{ResourceId: 1, Timestamp: 6, Duration: 5}, Entry{ResourceId: 0, Timestamp: 0, Duration: 5}, Entry{ResourceId: 1, Timestamp: 0, Duration: 2}, Entry{ResourceId: 2, Timestamp: 11, Duration: 1}, Entry{ResourceId: 4, Timestamp: (-1), Duration: 4}})
		expected := Report{Accepted: 4, Rejected: 3, ExpiresSum: 34}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 4, Timestamp: 6, Duration: 1}, Entry{ResourceId: 4, Timestamp: (-1), Duration: 1}, Entry{ResourceId: 0, Timestamp: (-3), Duration: 4}, Entry{ResourceId: 2, Timestamp: 9, Duration: 5}, Entry{ResourceId: 0, Timestamp: (-1), Duration: 2}, Entry{ResourceId: 0, Timestamp: 2, Duration: 2}, Entry{ResourceId: 1, Timestamp: (-2), Duration: 3}, Entry{ResourceId: 1, Timestamp: (-1), Duration: 6}})
		expected := Report{Accepted: 3, Rejected: 5, ExpiresSum: 25}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 0, Timestamp: 0, Duration: 0}, Entry{ResourceId: 0, Timestamp: 0, Duration: 2}, Entry{ResourceId: 0, Timestamp: 2, Duration: 3}, Entry{ResourceId: 0, Timestamp: 1, Duration: 9}})
		expected := Report{Accepted: 2, Rejected: 2, ExpiresSum: 5}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{ResourceId: 1, Timestamp: 0, Duration: 3}, Entry{ResourceId: 1, Timestamp: 2, Duration: 7}, Entry{ResourceId: 1, Timestamp: 3, Duration: 2}, Entry{ResourceId: 2, Timestamp: (-1), Duration: 4}})
		expected := Report{Accepted: 2, Rejected: 2, ExpiresSum: 5}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
}
