package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Accepted: 0, Rejected: 0, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Accepted: 0, Rejected: 0, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Accepted: 0, Rejected: 0, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 3, Seats: 8}}, 2)
		expected := Report{Accepted: 0, Rejected: 1, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 1, Seats: 6}, Entry{Party: 3, Seats: 7}}, 3)
		expected := Report{Accepted: 0, Rejected: 2, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 2, Seats: 11}, Entry{Party: 0, Seats: 0}, Entry{Party: 0, Seats: (-2)}}, 4)
		expected := Report{Accepted: 0, Rejected: 3, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 3, Seats: 10}, Entry{Party: 2, Seats: 2}, Entry{Party: 4, Seats: 0}, Entry{Party: 0, Seats: 11}}, 5)
		expected := Report{Accepted: 1, Rejected: 3, Occupied: 2}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 3, Seats: 10}, Entry{Party: 0, Seats: 5}, Entry{Party: 4, Seats: 2}, Entry{Party: 1, Seats: 3}, Entry{Party: 1, Seats: 1}}, 6)
		expected := Report{Accepted: 2, Rejected: 3, Occupied: 6}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 3, Seats: 5}, Entry{Party: 1, Seats: (-2)}, Entry{Party: 3, Seats: 11}, Entry{Party: 4, Seats: 2}, Entry{Party: 0, Seats: 10}, Entry{Party: 3, Seats: 6}}, 7)
		expected := Report{Accepted: 2, Rejected: 4, Occupied: 7}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 1, Seats: 5}, Entry{Party: 1, Seats: 1}, Entry{Party: 3, Seats: (-1)}, Entry{Party: 0, Seats: 8}, Entry{Party: 3, Seats: 7}, Entry{Party: 0, Seats: 5}, Entry{Party: 1, Seats: 11}}, 8)
		expected := Report{Accepted: 1, Rejected: 6, Occupied: 5}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 0, Seats: (-3)}, Entry{Party: 1, Seats: 7}, Entry{Party: 1, Seats: 7}, Entry{Party: 3, Seats: (-2)}, Entry{Party: 4, Seats: (-1)}, Entry{Party: 0, Seats: 4}, Entry{Party: 2, Seats: 8}, Entry{Party: 1, Seats: (-2)}}, 1)
		expected := Report{Accepted: 0, Rejected: 8, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 2, Seats: 10}, Entry{Party: 2, Seats: 11}, Entry{Party: 4, Seats: 2}, Entry{Party: 3, Seats: 1}, Entry{Party: 2, Seats: 1}, Entry{Party: 2, Seats: 4}, Entry{Party: 0, Seats: 2}, Entry{Party: 3, Seats: (-3)}, Entry{Party: 4, Seats: 2}}, 2)
		expected := Report{Accepted: 1, Rejected: 8, Occupied: 2}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 0, Seats: 7}, Entry{Party: 3, Seats: (-3)}, Entry{Party: 1, Seats: 5}, Entry{Party: 0, Seats: 10}, Entry{Party: 1, Seats: 8}, Entry{Party: 3, Seats: 6}, Entry{Party: 0, Seats: 2}, Entry{Party: 4, Seats: 5}, Entry{Party: 2, Seats: (-1)}, Entry{Party: 0, Seats: 1}}, 3)
		expected := Report{Accepted: 1, Rejected: 9, Occupied: 2}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 2, Seats: 5}, Entry{Party: 4, Seats: 3}, Entry{Party: 3, Seats: 11}, Entry{Party: 1, Seats: 8}, Entry{Party: 3, Seats: 4}, Entry{Party: 0, Seats: 1}, Entry{Party: 4, Seats: 3}, Entry{Party: 1, Seats: 5}, Entry{Party: 4, Seats: 6}, Entry{Party: 1, Seats: 9}, Entry{Party: 4, Seats: 7}}, 4)
		expected := Report{Accepted: 2, Rejected: 9, Occupied: 4}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 1, Seats: 3}, Entry{Party: 1, Seats: 7}, Entry{Party: 3, Seats: 4}, Entry{Party: 2, Seats: 6}, Entry{Party: 2, Seats: 11}, Entry{Party: 1, Seats: 0}, Entry{Party: 0, Seats: 2}, Entry{Party: 1, Seats: 10}, Entry{Party: 1, Seats: (-1)}, Entry{Party: 0, Seats: (-1)}, Entry{Party: 3, Seats: 11}, Entry{Party: 3, Seats: (-3)}}, 5)
		expected := Report{Accepted: 2, Rejected: 10, Occupied: 5}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Accepted: 0, Rejected: 0, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 3, Seats: (-1)}}, 7)
		expected := Report{Accepted: 0, Rejected: 1, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 0, Seats: 4}, Entry{Party: 4, Seats: 8}}, 8)
		expected := Report{Accepted: 1, Rejected: 1, Occupied: 4}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 3, Seats: 7}, Entry{Party: 0, Seats: (-1)}, Entry{Party: 0, Seats: 3}}, 1)
		expected := Report{Accepted: 0, Rejected: 3, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 1, Seats: 11}, Entry{Party: 0, Seats: 5}, Entry{Party: 3, Seats: 4}, Entry{Party: 0, Seats: 4}}, 2)
		expected := Report{Accepted: 0, Rejected: 4, Occupied: 0}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 1, Seats: 1}, Entry{Party: 2, Seats: 5}, Entry{Party: 4, Seats: (-3)}, Entry{Party: 2, Seats: 5}, Entry{Party: 1, Seats: (-3)}}, 3)
		expected := Report{Accepted: 1, Rejected: 4, Occupied: 1}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 0, Seats: 4}, Entry{Party: 2, Seats: 1}, Entry{Party: 3, Seats: 10}, Entry{Party: 4, Seats: (-3)}, Entry{Party: 2, Seats: 4}, Entry{Party: 1, Seats: 4}}, 4)
		expected := Report{Accepted: 1, Rejected: 5, Occupied: 4}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 2, Seats: 1}, Entry{Party: 0, Seats: 6}, Entry{Party: 1, Seats: 6}, Entry{Party: 0, Seats: 0}, Entry{Party: 1, Seats: 0}, Entry{Party: 2, Seats: 11}, Entry{Party: 4, Seats: (-1)}}, 5)
		expected := Report{Accepted: 1, Rejected: 6, Occupied: 1}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 4, Seats: 6}, Entry{Party: 4, Seats: (-1)}, Entry{Party: 0, Seats: (-3)}, Entry{Party: 2, Seats: 9}, Entry{Party: 0, Seats: (-1)}, Entry{Party: 0, Seats: 2}, Entry{Party: 1, Seats: (-2)}, Entry{Party: 1, Seats: (-1)}}, 6)
		expected := Report{Accepted: 1, Rejected: 7, Occupied: 6}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 0, Seats: 5}}, 5)
		expected := Report{Accepted: 1, Rejected: 0, Occupied: 5}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 0, Seats: 0}, Entry{Party: 0, Seats: 2}}, 2)
		expected := Report{Accepted: 1, Rejected: 1, Occupied: 2}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 1, Seats: 2}, Entry{Party: 2, Seats: 3}, Entry{Party: 1, Seats: 1}}, 5)
		expected := Report{Accepted: 2, Rejected: 1, Occupied: 5}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Party: 1, Seats: 6}, Entry{Party: 1, Seats: 3}, Entry{Party: 1, Seats: 1}, Entry{Party: 2, Seats: 2}}, 5)
		expected := Report{Accepted: 2, Rejected: 2, Occupied: 5}
		if actual != expected {
			t.Fatalf("fixture 27: got %+v expected %+v", actual, expected)
		}
	}
}
