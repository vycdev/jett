package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Assigned: 0, Rejected: 0, SeatChecksum: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Assigned: 0, Rejected: 0, SeatChecksum: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Assigned: 0, Rejected: 0, SeatChecksum: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 3, Preferred: 8}}, 2)
		expected := Report{Assigned: 1, Rejected: 0, SeatChecksum: 4}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 1, Preferred: 6}, Entry{Passenger: 3, Preferred: 7}}, 3)
		expected := Report{Assigned: 2, Rejected: 0, SeatChecksum: 10}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 2, Preferred: 11}, Entry{Passenger: 0, Preferred: 0}, Entry{Passenger: 0, Preferred: (-2)}}, 4)
		expected := Report{Assigned: 2, Rejected: 1, SeatChecksum: 5}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 3, Preferred: 10}, Entry{Passenger: 2, Preferred: 2}, Entry{Passenger: 4, Preferred: 0}, Entry{Passenger: 0, Preferred: 11}}, 5)
		expected := Report{Assigned: 4, Rejected: 0, SeatChecksum: 29}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 3, Preferred: 10}, Entry{Passenger: 0, Preferred: 5}, Entry{Passenger: 4, Preferred: 2}, Entry{Passenger: 1, Preferred: 3}, Entry{Passenger: 1, Preferred: 1}}, 6)
		expected := Report{Assigned: 4, Rejected: 1, SeatChecksum: 25}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 3, Preferred: 5}, Entry{Passenger: 1, Preferred: (-2)}, Entry{Passenger: 3, Preferred: 11}, Entry{Passenger: 4, Preferred: 2}, Entry{Passenger: 0, Preferred: 10}, Entry{Passenger: 3, Preferred: 6}}, 7)
		expected := Report{Assigned: 4, Rejected: 2, SeatChecksum: 35}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 1, Preferred: 5}, Entry{Passenger: 1, Preferred: 1}, Entry{Passenger: 3, Preferred: (-1)}, Entry{Passenger: 0, Preferred: 8}, Entry{Passenger: 3, Preferred: 7}, Entry{Passenger: 0, Preferred: 5}, Entry{Passenger: 1, Preferred: 11}}, 8)
		expected := Report{Assigned: 3, Rejected: 4, SeatChecksum: 22}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 0, Preferred: (-3)}, Entry{Passenger: 1, Preferred: 7}, Entry{Passenger: 1, Preferred: 7}, Entry{Passenger: 3, Preferred: (-2)}, Entry{Passenger: 4, Preferred: (-1)}, Entry{Passenger: 0, Preferred: 4}, Entry{Passenger: 2, Preferred: 8}, Entry{Passenger: 1, Preferred: (-2)}}, 1)
		expected := Report{Assigned: 1, Rejected: 7, SeatChecksum: 1}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 2, Preferred: 10}, Entry{Passenger: 2, Preferred: 11}, Entry{Passenger: 4, Preferred: 2}, Entry{Passenger: 3, Preferred: 1}, Entry{Passenger: 2, Preferred: 1}, Entry{Passenger: 2, Preferred: 4}, Entry{Passenger: 0, Preferred: 2}, Entry{Passenger: 3, Preferred: (-3)}, Entry{Passenger: 4, Preferred: 2}}, 2)
		expected := Report{Assigned: 2, Rejected: 7, SeatChecksum: 13}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 0, Preferred: 7}, Entry{Passenger: 3, Preferred: (-3)}, Entry{Passenger: 1, Preferred: 5}, Entry{Passenger: 0, Preferred: 10}, Entry{Passenger: 1, Preferred: 8}, Entry{Passenger: 3, Preferred: 6}, Entry{Passenger: 0, Preferred: 2}, Entry{Passenger: 4, Preferred: 5}, Entry{Passenger: 2, Preferred: (-1)}, Entry{Passenger: 0, Preferred: 1}}, 3)
		expected := Report{Assigned: 3, Rejected: 7, SeatChecksum: 15}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 2, Preferred: 5}, Entry{Passenger: 4, Preferred: 3}, Entry{Passenger: 3, Preferred: 11}, Entry{Passenger: 1, Preferred: 8}, Entry{Passenger: 3, Preferred: 4}, Entry{Passenger: 0, Preferred: 1}, Entry{Passenger: 4, Preferred: 3}, Entry{Passenger: 1, Preferred: 5}, Entry{Passenger: 4, Preferred: 6}, Entry{Passenger: 1, Preferred: 9}, Entry{Passenger: 4, Preferred: 7}}, 4)
		expected := Report{Assigned: 4, Rejected: 7, SeatChecksum: 34}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 1, Preferred: 3}, Entry{Passenger: 1, Preferred: 7}, Entry{Passenger: 3, Preferred: 4}, Entry{Passenger: 2, Preferred: 6}, Entry{Passenger: 2, Preferred: 11}, Entry{Passenger: 1, Preferred: 0}, Entry{Passenger: 0, Preferred: 2}, Entry{Passenger: 1, Preferred: 10}, Entry{Passenger: 1, Preferred: (-1)}, Entry{Passenger: 0, Preferred: (-1)}, Entry{Passenger: 3, Preferred: 11}, Entry{Passenger: 3, Preferred: (-3)}}, 5)
		expected := Report{Assigned: 4, Rejected: 8, SeatChecksum: 27}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Assigned: 0, Rejected: 0, SeatChecksum: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 3, Preferred: (-1)}}, 7)
		expected := Report{Assigned: 1, Rejected: 0, SeatChecksum: 4}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 0, Preferred: 4}, Entry{Passenger: 4, Preferred: 8}}, 8)
		expected := Report{Assigned: 2, Rejected: 0, SeatChecksum: 44}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 3, Preferred: 7}, Entry{Passenger: 0, Preferred: (-1)}, Entry{Passenger: 0, Preferred: 3}}, 1)
		expected := Report{Assigned: 1, Rejected: 2, SeatChecksum: 4}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 1, Preferred: 11}, Entry{Passenger: 0, Preferred: 5}, Entry{Passenger: 3, Preferred: 4}, Entry{Passenger: 0, Preferred: 4}}, 2)
		expected := Report{Assigned: 2, Rejected: 2, SeatChecksum: 4}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 1, Preferred: 1}, Entry{Passenger: 2, Preferred: 5}, Entry{Passenger: 4, Preferred: (-3)}, Entry{Passenger: 2, Preferred: 5}, Entry{Passenger: 1, Preferred: (-3)}}, 3)
		expected := Report{Assigned: 3, Rejected: 2, SeatChecksum: 23}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 0, Preferred: 4}, Entry{Passenger: 2, Preferred: 1}, Entry{Passenger: 3, Preferred: 10}, Entry{Passenger: 4, Preferred: (-3)}, Entry{Passenger: 2, Preferred: 4}, Entry{Passenger: 1, Preferred: 4}}, 4)
		expected := Report{Assigned: 4, Rejected: 2, SeatChecksum: 30}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 2, Preferred: 1}, Entry{Passenger: 0, Preferred: 6}, Entry{Passenger: 1, Preferred: 6}, Entry{Passenger: 0, Preferred: 0}, Entry{Passenger: 1, Preferred: 0}, Entry{Passenger: 2, Preferred: 11}, Entry{Passenger: 4, Preferred: (-1)}}, 5)
		expected := Report{Assigned: 4, Rejected: 3, SeatChecksum: 31}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 4, Preferred: 6}, Entry{Passenger: 4, Preferred: (-1)}, Entry{Passenger: 0, Preferred: (-3)}, Entry{Passenger: 2, Preferred: 9}, Entry{Passenger: 0, Preferred: (-1)}, Entry{Passenger: 0, Preferred: 2}, Entry{Passenger: 1, Preferred: (-2)}, Entry{Passenger: 1, Preferred: (-1)}}, 6)
		expected := Report{Assigned: 4, Rejected: 4, SeatChecksum: 43}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 0, Preferred: 0}, Entry{Passenger: 1, Preferred: (-1)}, Entry{Passenger: 2, Preferred: 9}, Entry{Passenger: 0, Preferred: 3}}, 3)
		expected := Report{Assigned: 3, Rejected: 1, SeatChecksum: 14}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Passenger: 2, Preferred: 2}, Entry{Passenger: 3, Preferred: 2}, Entry{Passenger: 2, Preferred: 3}, Entry{Passenger: 4, Preferred: 9}, Entry{Passenger: 5, Preferred: 1}}, 3)
		expected := Report{Assigned: 3, Rejected: 2, SeatChecksum: 25}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
}
