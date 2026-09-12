package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Accepted: 0, Stale: 0, Checksum: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Version: 8, Value: 3}})
		expected := Report{Accepted: 1, Stale: 0, Checksum: 12}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Version: 6, Value: 3}, Entry{Key: 3, Version: 7, Value: 5}})
		expected := Report{Accepted: 2, Stale: 0, Checksum: 26}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Version: 11, Value: 1}, Entry{Key: 0, Version: 0, Value: 6}, Entry{Key: 0, Version: (-2), Value: 5}})
		expected := Report{Accepted: 2, Stale: 1, Checksum: 9}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Version: 10, Value: 2}, Entry{Key: 2, Version: 2, Value: 3}, Entry{Key: 4, Version: 0, Value: 4}, Entry{Key: 0, Version: 11, Value: 2}})
		expected := Report{Accepted: 4, Stale: 0, Checksum: 39}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Version: 10, Value: 3}, Entry{Key: 0, Version: 5, Value: 5}, Entry{Key: 4, Version: 2, Value: 6}, Entry{Key: 1, Version: 3, Value: 1}, Entry{Key: 1, Version: 1, Value: 1}})
		expected := Report{Accepted: 4, Stale: 1, Checksum: 49}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Version: 5, Value: 6}, Entry{Key: 1, Version: (-2), Value: 5}, Entry{Key: 3, Version: 11, Value: 6}, Entry{Key: 4, Version: 2, Value: 4}, Entry{Key: 0, Version: 10, Value: 6}, Entry{Key: 3, Version: 6, Value: 5}})
		expected := Report{Accepted: 4, Stale: 2, Checksum: 50}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Version: 5, Value: 1}, Entry{Key: 1, Version: 1, Value: 1}, Entry{Key: 3, Version: (-1), Value: 1}, Entry{Key: 0, Version: 8, Value: 4}, Entry{Key: 3, Version: 7, Value: 4}, Entry{Key: 0, Version: 5, Value: 4}, Entry{Key: 1, Version: 11, Value: 3}})
		expected := Report{Accepted: 4, Stale: 3, Checksum: 26}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Version: (-3), Value: 3}, Entry{Key: 1, Version: 7, Value: 4}, Entry{Key: 1, Version: 7, Value: 1}, Entry{Key: 3, Version: (-2), Value: 3}, Entry{Key: 4, Version: (-1), Value: 4}, Entry{Key: 0, Version: 4, Value: 5}, Entry{Key: 2, Version: 8, Value: 4}, Entry{Key: 1, Version: (-2), Value: 6}})
		expected := Report{Accepted: 3, Stale: 5, Checksum: 25}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Version: 10, Value: 2}, Entry{Key: 2, Version: 11, Value: 2}, Entry{Key: 4, Version: 2, Value: 2}, Entry{Key: 3, Version: 1, Value: 2}, Entry{Key: 2, Version: 1, Value: 5}, Entry{Key: 2, Version: 4, Value: 2}, Entry{Key: 0, Version: 2, Value: 1}, Entry{Key: 3, Version: (-3), Value: 4}, Entry{Key: 4, Version: 2, Value: 2}})
		expected := Report{Accepted: 5, Stale: 4, Checksum: 25}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Version: 7, Value: 1}, Entry{Key: 3, Version: (-3), Value: 1}, Entry{Key: 1, Version: 5, Value: 6}, Entry{Key: 0, Version: 10, Value: 1}, Entry{Key: 1, Version: 8, Value: 3}, Entry{Key: 3, Version: 6, Value: 3}, Entry{Key: 0, Version: 2, Value: 3}, Entry{Key: 4, Version: 5, Value: 3}, Entry{Key: 2, Version: (-1), Value: 2}, Entry{Key: 0, Version: 1, Value: 2}})
		expected := Report{Accepted: 6, Stale: 4, Checksum: 34}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Version: 5, Value: 3}, Entry{Key: 4, Version: 3, Value: 6}, Entry{Key: 3, Version: 11, Value: 1}, Entry{Key: 1, Version: 8, Value: 6}, Entry{Key: 3, Version: 4, Value: 2}, Entry{Key: 0, Version: 1, Value: 1}, Entry{Key: 4, Version: 3, Value: 2}, Entry{Key: 1, Version: 5, Value: 6}, Entry{Key: 4, Version: 6, Value: 5}, Entry{Key: 1, Version: 9, Value: 5}, Entry{Key: 4, Version: 7, Value: 2}})
		expected := Report{Accepted: 8, Stale: 3, Checksum: 34}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Version: 3, Value: 2}, Entry{Key: 1, Version: 7, Value: 2}, Entry{Key: 3, Version: 4, Value: 6}, Entry{Key: 2, Version: 6, Value: 3}, Entry{Key: 2, Version: 11, Value: 3}, Entry{Key: 1, Version: 0, Value: 5}, Entry{Key: 0, Version: 2, Value: 2}, Entry{Key: 1, Version: 10, Value: 3}, Entry{Key: 1, Version: (-1), Value: 5}, Entry{Key: 0, Version: (-1), Value: 1}, Entry{Key: 3, Version: 11, Value: 6}, Entry{Key: 3, Version: (-3), Value: 4}})
		expected := Report{Accepted: 8, Stale: 4, Checksum: 41}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Version: (-1), Value: 5}})
		expected := Report{Accepted: 0, Stale: 1, Checksum: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Version: 4, Value: 1}, Entry{Key: 4, Version: 8, Value: 4}})
		expected := Report{Accepted: 2, Stale: 0, Checksum: 21}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 3, Version: 7, Value: 3}, Entry{Key: 0, Version: (-1), Value: 4}, Entry{Key: 0, Version: 3, Value: 4}})
		expected := Report{Accepted: 2, Stale: 1, Checksum: 16}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Version: 11, Value: 3}, Entry{Key: 0, Version: 5, Value: 2}, Entry{Key: 3, Version: 4, Value: 3}, Entry{Key: 0, Version: 4, Value: 2}})
		expected := Report{Accepted: 3, Stale: 1, Checksum: 20}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Version: 1, Value: 5}, Entry{Key: 2, Version: 5, Value: 4}, Entry{Key: 4, Version: (-3), Value: 3}, Entry{Key: 2, Version: 5, Value: 5}, Entry{Key: 1, Version: (-3), Value: 6}})
		expected := Report{Accepted: 2, Stale: 3, Checksum: 22}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Version: 4, Value: 4}, Entry{Key: 2, Version: 1, Value: 6}, Entry{Key: 3, Version: 10, Value: 1}, Entry{Key: 4, Version: (-3), Value: 1}, Entry{Key: 2, Version: 4, Value: 1}, Entry{Key: 1, Version: 4, Value: 6}})
		expected := Report{Accepted: 5, Stale: 1, Checksum: 23}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 2, Version: 1, Value: 6}, Entry{Key: 0, Version: 6, Value: 5}, Entry{Key: 1, Version: 6, Value: 5}, Entry{Key: 0, Version: 0, Value: 5}, Entry{Key: 1, Version: 0, Value: 2}, Entry{Key: 2, Version: 11, Value: 1}, Entry{Key: 4, Version: (-1), Value: 4}})
		expected := Report{Accepted: 4, Stale: 3, Checksum: 18}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 4, Version: 6, Value: 1}, Entry{Key: 4, Version: (-1), Value: 1}, Entry{Key: 0, Version: (-3), Value: 4}, Entry{Key: 2, Version: 9, Value: 5}, Entry{Key: 0, Version: (-1), Value: 2}, Entry{Key: 0, Version: 2, Value: 2}, Entry{Key: 1, Version: (-2), Value: 3}, Entry{Key: 1, Version: (-1), Value: 6}})
		expected := Report{Accepted: 3, Stale: 5, Checksum: 22}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Version: 0, Value: 3}, Entry{Key: 0, Version: 0, Value: 9}})
		expected := Report{Accepted: 1, Stale: 1, Checksum: 3}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 0, Version: (-1), Value: 8}, Entry{Key: 0, Version: (-2), Value: 9}, Entry{Key: 0, Version: 0, Value: 0}})
		expected := Report{Accepted: 1, Stale: 2, Checksum: 0}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Key: 1, Version: 0, Value: 8}, Entry{Key: 1, Version: 0, Value: 9}, Entry{Key: 1, Version: 2, Value: (-3)}, Entry{Key: 2, Version: (-1), Value: 5}})
		expected := Report{Accepted: 2, Stale: 2, Checksum: (-6)}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
}
