package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{})
		expected := Report{Rooms: 0, AssignmentChecksum: 0, Reuses: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 11, End: 15}})
		expected := Report{Rooms: 1, AssignmentChecksum: 1, Reuses: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 8}, Entry{Start: 6, End: 12}})
		expected := Report{Rooms: 2, AssignmentChecksum: 5, Reuses: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 2}, Entry{Start: 1, End: 6}, Entry{Start: 10, End: 13}})
		expected := Report{Rooms: 2, AssignmentChecksum: 8, Reuses: 1}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 6}, Entry{Start: 5, End: 10}, Entry{Start: 6, End: 13}, Entry{Start: 7, End: 8}})
		expected := Report{Rooms: 3, AssignmentChecksum: 20, Reuses: 1}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 5}, Entry{Start: 4, End: 5}, Entry{Start: 6, End: 7}, Entry{Start: 8, End: 15}, Entry{Start: 9, End: 12}})
		expected := Report{Rooms: 2, AssignmentChecksum: 22, Reuses: 3}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 2}, Entry{Start: 1, End: 8}, Entry{Start: 5, End: 9}, Entry{Start: 7, End: 13}, Entry{Start: 9, End: 11}, Entry{Start: 10, End: 14}})
		expected := Report{Rooms: 3, AssignmentChecksum: 37, Reuses: 3}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 1}, Entry{Start: 0, End: 3}, Entry{Start: 1, End: 5}, Entry{Start: 5, End: 12}, Entry{Start: 6, End: 12}, Entry{Start: 7, End: 14}, Entry{Start: 11, End: 12}})
		expected := Report{Rooms: 4, AssignmentChecksum: 68, Reuses: 3}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 2, End: 3}, Entry{Start: 3, End: 9}, Entry{Start: 7, End: 8}, Entry{Start: 9, End: 11}, Entry{Start: 9, End: 12}, Entry{Start: 10, End: 14}, Entry{Start: 10, End: 17}, Entry{Start: 11, End: 12}})
		expected := Report{Rooms: 4, AssignmentChecksum: 77, Reuses: 4}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 4}, Entry{Start: 2, End: 9}, Entry{Start: 3, End: 4}, Entry{Start: 4, End: 6}, Entry{Start: 4, End: 11}, Entry{Start: 5, End: 7}, Entry{Start: 5, End: 9}, Entry{Start: 6, End: 11}, Entry{Start: 10, End: 11}})
		expected := Report{Rooms: 5, AssignmentChecksum: 118, Reuses: 4}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 7}, Entry{Start: 2, End: 7}, Entry{Start: 2, End: 4}, Entry{Start: 4, End: 6}, Entry{Start: 5, End: 8}, Entry{Start: 7, End: 8}, Entry{Start: 8, End: 11}, Entry{Start: 8, End: 11}, Entry{Start: 9, End: 12}, Entry{Start: 11, End: 14}})
		expected := Report{Rooms: 4, AssignmentChecksum: 112, Reuses: 6}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 2}, Entry{Start: 2, End: 8}, Entry{Start: 3, End: 7}, Entry{Start: 4, End: 5}, Entry{Start: 6, End: 12}, Entry{Start: 6, End: 8}, Entry{Start: 7, End: 9}, Entry{Start: 7, End: 11}, Entry{Start: 8, End: 14}, Entry{Start: 9, End: 14}, Entry{Start: 9, End: 14}})
		expected := Report{Rooms: 5, AssignmentChecksum: 187, Reuses: 6}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 8}, Entry{Start: 1, End: 6}, Entry{Start: 3, End: 8}, Entry{Start: 4, End: 6}, Entry{Start: 5, End: 12}, Entry{Start: 5, End: 7}, Entry{Start: 6, End: 10}, Entry{Start: 6, End: 12}, Entry{Start: 8, End: 9}, Entry{Start: 9, End: 12}, Entry{Start: 9, End: 10}, Entry{Start: 11, End: 15}})
		expected := Report{Rooms: 6, AssignmentChecksum: 213, Reuses: 6}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 1, End: 3}})
		expected := Report{Rooms: 1, AssignmentChecksum: 1, Reuses: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 4}, Entry{Start: 2, End: 5}})
		expected := Report{Rooms: 2, AssignmentChecksum: 5, Reuses: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 7, End: 10}, Entry{Start: 7, End: 9}, Entry{Start: 8, End: 10}})
		expected := Report{Rooms: 3, AssignmentChecksum: 14, Reuses: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 3}, Entry{Start: 4, End: 9}, Entry{Start: 8, End: 12}, Entry{Start: 8, End: 15}})
		expected := Report{Rooms: 3, AssignmentChecksum: 21, Reuses: 1}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 6, End: 9}, Entry{Start: 8, End: 9}, Entry{Start: 10, End: 12}, Entry{Start: 10, End: 11}, Entry{Start: 10, End: 14}})
		expected := Report{Rooms: 3, AssignmentChecksum: 31, Reuses: 2}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 3, End: 7}, Entry{Start: 4, End: 10}, Entry{Start: 5, End: 9}, Entry{Start: 9, End: 16}, Entry{Start: 9, End: 16}, Entry{Start: 10, End: 12}})
		expected := Report{Rooms: 3, AssignmentChecksum: 45, Reuses: 3}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 6}, Entry{Start: 2, End: 9}, Entry{Start: 3, End: 8}, Entry{Start: 3, End: 5}, Entry{Start: 8, End: 13}, Entry{Start: 8, End: 10}, Entry{Start: 10, End: 11}})
		expected := Report{Rooms: 4, AssignmentChecksum: 67, Reuses: 3}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 2, End: 9}, Entry{Start: 3, End: 5}, Entry{Start: 4, End: 9}, Entry{Start: 5, End: 7}, Entry{Start: 7, End: 14}, Entry{Start: 8, End: 15}, Entry{Start: 10, End: 11}, Entry{Start: 10, End: 12}})
		expected := Report{Rooms: 4, AssignmentChecksum: 87, Reuses: 4}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 1}, Entry{Start: 0, End: 2}, Entry{Start: 0, End: 3}, Entry{Start: 3, End: 4}, Entry{Start: 3, End: 4}})
		expected := Report{Rooms: 3, AssignmentChecksum: 28, Reuses: 2}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Start: 0, End: 5}, Entry{Start: 1, End: 3}, Entry{Start: 3, End: 6}, Entry{Start: 5, End: 7}})
		expected := Report{Rooms: 2, AssignmentChecksum: 15, Reuses: 2}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
}
