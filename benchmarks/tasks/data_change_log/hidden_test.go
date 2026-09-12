package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Applied: 0, Duplicates: 0, Checksum: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 3, Key: 2, Delta: 1}})
		expected := Report{Applied: 1, Duplicates: 0, Checksum: 3}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 4, Key: 2, Delta: 4}, Entry{Event: 4, Key: 2, Delta: (-2)}})
		expected := Report{Applied: 1, Duplicates: 1, Checksum: 12}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 0, Key: 1, Delta: 8}, Entry{Event: 0, Key: 0, Delta: 6}, Entry{Event: 3, Key: 1, Delta: 3}})
		expected := Report{Applied: 2, Duplicates: 1, Checksum: 22}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 2, Key: 2, Delta: 7}, Entry{Event: 1, Key: 3, Delta: (-1)}, Entry{Event: 1, Key: 3, Delta: 3}, Entry{Event: 0, Key: 2, Delta: 9}})
		expected := Report{Applied: 3, Duplicates: 1, Checksum: 44}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 1, Key: 3, Delta: (-2)}, Entry{Event: 1, Key: 2, Delta: (-1)}, Entry{Event: 3, Key: 1, Delta: (-1)}, Entry{Event: 4, Key: 3, Delta: 9}, Entry{Event: 4, Key: 2, Delta: 5}})
		expected := Report{Applied: 3, Duplicates: 2, Checksum: 26}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 0, Key: 3, Delta: 7}, Entry{Event: 4, Key: 1, Delta: 6}, Entry{Event: 0, Key: 1, Delta: 2}, Entry{Event: 0, Key: 3, Delta: 0}, Entry{Event: 0, Key: 0, Delta: 9}, Entry{Event: 3, Key: 3, Delta: 8}})
		expected := Report{Applied: 3, Duplicates: 3, Checksum: 72}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 3, Key: 0, Delta: 6}, Entry{Event: 3, Key: 1, Delta: 3}, Entry{Event: 0, Key: 0, Delta: 2}, Entry{Event: 1, Key: 3, Delta: 0}, Entry{Event: 0, Key: 3, Delta: (-1)}, Entry{Event: 2, Key: 1, Delta: 4}, Entry{Event: 0, Key: 3, Delta: 7}})
		expected := Report{Applied: 4, Duplicates: 3, Checksum: 16}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 2, Key: 3, Delta: 0}, Entry{Event: 0, Key: 2, Delta: 1}, Entry{Event: 2, Key: 1, Delta: 7}, Entry{Event: 2, Key: 1, Delta: 4}, Entry{Event: 2, Key: 1, Delta: 2}, Entry{Event: 2, Key: 2, Delta: 5}, Entry{Event: 1, Key: 0, Delta: 3}, Entry{Event: 0, Key: 3, Delta: (-2)}})
		expected := Report{Applied: 3, Duplicates: 5, Checksum: 6}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 3, Key: 2, Delta: 1}, Entry{Event: 0, Key: 0, Delta: 5}, Entry{Event: 0, Key: 0, Delta: 0}, Entry{Event: 4, Key: 0, Delta: (-1)}, Entry{Event: 1, Key: 2, Delta: 4}, Entry{Event: 4, Key: 2, Delta: 9}, Entry{Event: 0, Key: 2, Delta: 2}, Entry{Event: 4, Key: 2, Delta: 3}, Entry{Event: 1, Key: 1, Delta: (-1)}})
		expected := Report{Applied: 4, Duplicates: 5, Checksum: 19}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 2, Key: 1, Delta: 3}, Entry{Event: 4, Key: 2, Delta: 6}, Entry{Event: 3, Key: 3, Delta: (-2)}, Entry{Event: 1, Key: 3, Delta: 5}, Entry{Event: 1, Key: 0, Delta: 2}, Entry{Event: 0, Key: 3, Delta: 1}, Entry{Event: 1, Key: 1, Delta: 7}, Entry{Event: 4, Key: 1, Delta: 1}, Entry{Event: 3, Key: 1, Delta: 0}, Entry{Event: 1, Key: 3, Delta: 5}})
		expected := Report{Applied: 5, Duplicates: 5, Checksum: 40}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 2, Key: 2, Delta: 2}, Entry{Event: 2, Key: 1, Delta: 1}, Entry{Event: 4, Key: 0, Delta: 3}, Entry{Event: 1, Key: 1, Delta: 2}, Entry{Event: 1, Key: 1, Delta: 7}, Entry{Event: 0, Key: 1, Delta: (-1)}, Entry{Event: 3, Key: 3, Delta: (-2)}, Entry{Event: 3, Key: 3, Delta: 0}, Entry{Event: 4, Key: 0, Delta: 5}, Entry{Event: 0, Key: 3, Delta: 4}, Entry{Event: 2, Key: 0, Delta: 0}})
		expected := Report{Applied: 5, Duplicates: 6, Checksum: 3}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 3, Key: 0, Delta: 4}, Entry{Event: 3, Key: 1, Delta: 3}, Entry{Event: 0, Key: 1, Delta: 5}, Entry{Event: 3, Key: 2, Delta: (-2)}, Entry{Event: 3, Key: 1, Delta: 1}, Entry{Event: 2, Key: 2, Delta: 6}, Entry{Event: 3, Key: 0, Delta: 2}, Entry{Event: 2, Key: 1, Delta: (-2)}, Entry{Event: 0, Key: 3, Delta: 4}, Entry{Event: 2, Key: 2, Delta: 8}, Entry{Event: 3, Key: 0, Delta: 6}, Entry{Event: 0, Key: 0, Delta: 3}})
		expected := Report{Applied: 3, Duplicates: 9, Checksum: 32}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 3, Key: 0, Delta: 1}})
		expected := Report{Applied: 1, Duplicates: 0, Checksum: 1}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 3, Key: 2, Delta: 2}, Entry{Event: 0, Key: 1, Delta: 7}})
		expected := Report{Applied: 2, Duplicates: 0, Checksum: 20}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 4, Key: 0, Delta: 1}, Entry{Event: 4, Key: 1, Delta: 1}, Entry{Event: 1, Key: 2, Delta: (-2)}})
		expected := Report{Applied: 2, Duplicates: 1, Checksum: (-5)}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 4, Key: 1, Delta: 5}, Entry{Event: 4, Key: 0, Delta: 6}, Entry{Event: 1, Key: 0, Delta: 8}, Entry{Event: 0, Key: 0, Delta: 5}})
		expected := Report{Applied: 3, Duplicates: 1, Checksum: 23}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 2, Key: 0, Delta: 0}, Entry{Event: 1, Key: 0, Delta: 3}, Entry{Event: 1, Key: 1, Delta: (-1)}, Entry{Event: 2, Key: 1, Delta: 0}, Entry{Event: 1, Key: 2, Delta: 2}})
		expected := Report{Applied: 2, Duplicates: 3, Checksum: 3}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 4, Key: 3, Delta: (-2)}, Entry{Event: 1, Key: 3, Delta: 6}, Entry{Event: 3, Key: 0, Delta: 2}, Entry{Event: 4, Key: 1, Delta: (-2)}, Entry{Event: 4, Key: 3, Delta: (-2)}, Entry{Event: 3, Key: 0, Delta: 0}})
		expected := Report{Applied: 3, Duplicates: 3, Checksum: 18}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 1, Key: 0, Delta: 5}, Entry{Event: 3, Key: 2, Delta: (-1)}, Entry{Event: 3, Key: 3, Delta: 5}, Entry{Event: 0, Key: 3, Delta: (-2)}, Entry{Event: 4, Key: 1, Delta: (-1)}, Entry{Event: 1, Key: 0, Delta: 3}, Entry{Event: 2, Key: 3, Delta: (-1)}})
		expected := Report{Applied: 5, Duplicates: 2, Checksum: (-12)}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 2, Key: 3, Delta: (-1)}, Entry{Event: 2, Key: 0, Delta: (-1)}, Entry{Event: 2, Key: 2, Delta: 5}, Entry{Event: 1, Key: 3, Delta: 3}, Entry{Event: 4, Key: 2, Delta: 6}, Entry{Event: 3, Key: 0, Delta: 1}, Entry{Event: 3, Key: 0, Delta: 1}, Entry{Event: 3, Key: 0, Delta: 9}})
		expected := Report{Applied: 4, Duplicates: 4, Checksum: 27}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 0, Key: 0, Delta: 0}, Entry{Event: 0, Key: 1, Delta: 99}, Entry{Event: 1, Key: 0, Delta: (-1)}})
		expected := Report{Applied: 2, Duplicates: 1, Checksum: (-1)}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Event: 1, Key: 0, Delta: 5}, Entry{Event: 2, Key: 1, Delta: (-2)}, Entry{Event: 1, Key: 9, Delta: 9}, Entry{Event: 3, Key: 0, Delta: 1}})
		expected := Report{Applied: 3, Duplicates: 1, Checksum: 2}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
}
