package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 3, Choice: 2, Weight: 1}}, 2)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 4, Choice: 2, Weight: 4}, Entry{Voter: 4, Choice: 2, Weight: (-2)}}, 3)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 0, Choice: 1, Weight: 8}, Entry{Voter: 0, Choice: 0, Weight: 6}, Entry{Voter: 3, Choice: 1, Weight: 3}}, 4)
		expected := Report{YesWeight: 3, NoWeight: 6, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 2, Choice: 2, Weight: 7}, Entry{Voter: 1, Choice: 3, Weight: (-1)}, Entry{Voter: 1, Choice: 3, Weight: 3}, Entry{Voter: 0, Choice: 2, Weight: 9}}, 5)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 1, Choice: 3, Weight: (-2)}, Entry{Voter: 1, Choice: 2, Weight: (-1)}, Entry{Voter: 3, Choice: 1, Weight: (-1)}, Entry{Voter: 4, Choice: 3, Weight: 9}, Entry{Voter: 4, Choice: 2, Weight: 5}}, 6)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 0, Choice: 3, Weight: 7}, Entry{Voter: 4, Choice: 1, Weight: 6}, Entry{Voter: 0, Choice: 1, Weight: 2}, Entry{Voter: 0, Choice: 3, Weight: 0}, Entry{Voter: 0, Choice: 0, Weight: 9}, Entry{Voter: 3, Choice: 3, Weight: 8}}, 7)
		expected := Report{YesWeight: 6, NoWeight: 9, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 3, Choice: 0, Weight: 6}, Entry{Voter: 3, Choice: 1, Weight: 3}, Entry{Voter: 0, Choice: 0, Weight: 2}, Entry{Voter: 1, Choice: 3, Weight: 0}, Entry{Voter: 0, Choice: 3, Weight: (-1)}, Entry{Voter: 2, Choice: 1, Weight: 4}, Entry{Voter: 0, Choice: 3, Weight: 7}}, 8)
		expected := Report{YesWeight: 7, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 2, Choice: 3, Weight: 0}, Entry{Voter: 0, Choice: 2, Weight: 1}, Entry{Voter: 2, Choice: 1, Weight: 7}, Entry{Voter: 2, Choice: 1, Weight: 4}, Entry{Voter: 2, Choice: 1, Weight: 2}, Entry{Voter: 2, Choice: 2, Weight: 5}, Entry{Voter: 1, Choice: 0, Weight: 3}, Entry{Voter: 0, Choice: 3, Weight: (-2)}}, 1)
		expected := Report{YesWeight: 0, NoWeight: 3, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 3, Choice: 2, Weight: 1}, Entry{Voter: 0, Choice: 0, Weight: 5}, Entry{Voter: 0, Choice: 0, Weight: 0}, Entry{Voter: 4, Choice: 0, Weight: (-1)}, Entry{Voter: 1, Choice: 2, Weight: 4}, Entry{Voter: 4, Choice: 2, Weight: 9}, Entry{Voter: 0, Choice: 2, Weight: 2}, Entry{Voter: 4, Choice: 2, Weight: 3}, Entry{Voter: 1, Choice: 1, Weight: (-1)}}, 2)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 2, Choice: 1, Weight: 3}, Entry{Voter: 4, Choice: 2, Weight: 6}, Entry{Voter: 3, Choice: 3, Weight: (-2)}, Entry{Voter: 1, Choice: 3, Weight: 5}, Entry{Voter: 1, Choice: 0, Weight: 2}, Entry{Voter: 0, Choice: 3, Weight: 1}, Entry{Voter: 1, Choice: 1, Weight: 7}, Entry{Voter: 4, Choice: 1, Weight: 1}, Entry{Voter: 3, Choice: 1, Weight: 0}, Entry{Voter: 1, Choice: 3, Weight: 5}}, 3)
		expected := Report{YesWeight: 4, NoWeight: 0, QuorumMet: 1}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 2, Choice: 2, Weight: 2}, Entry{Voter: 2, Choice: 1, Weight: 1}, Entry{Voter: 4, Choice: 0, Weight: 3}, Entry{Voter: 1, Choice: 1, Weight: 2}, Entry{Voter: 1, Choice: 1, Weight: 7}, Entry{Voter: 0, Choice: 1, Weight: (-1)}, Entry{Voter: 3, Choice: 3, Weight: (-2)}, Entry{Voter: 3, Choice: 3, Weight: 0}, Entry{Voter: 4, Choice: 0, Weight: 5}, Entry{Voter: 0, Choice: 3, Weight: 4}, Entry{Voter: 2, Choice: 0, Weight: 0}}, 4)
		expected := Report{YesWeight: 7, NoWeight: 5, QuorumMet: 1}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 3, Choice: 0, Weight: 4}, Entry{Voter: 3, Choice: 1, Weight: 3}, Entry{Voter: 0, Choice: 1, Weight: 5}, Entry{Voter: 3, Choice: 2, Weight: (-2)}, Entry{Voter: 3, Choice: 1, Weight: 1}, Entry{Voter: 2, Choice: 2, Weight: 6}, Entry{Voter: 3, Choice: 0, Weight: 2}, Entry{Voter: 2, Choice: 1, Weight: (-2)}, Entry{Voter: 0, Choice: 3, Weight: 4}, Entry{Voter: 2, Choice: 2, Weight: 8}, Entry{Voter: 3, Choice: 0, Weight: 6}, Entry{Voter: 0, Choice: 0, Weight: 3}}, 5)
		expected := Report{YesWeight: 0, NoWeight: 9, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 3, Choice: 0, Weight: 1}}, 7)
		expected := Report{YesWeight: 0, NoWeight: 1, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 3, Choice: 2, Weight: 2}, Entry{Voter: 0, Choice: 1, Weight: 7}}, 8)
		expected := Report{YesWeight: 7, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 4, Choice: 0, Weight: 1}, Entry{Voter: 4, Choice: 1, Weight: 1}, Entry{Voter: 1, Choice: 2, Weight: (-2)}}, 1)
		expected := Report{YesWeight: 1, NoWeight: 0, QuorumMet: 1}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 4, Choice: 1, Weight: 5}, Entry{Voter: 4, Choice: 0, Weight: 6}, Entry{Voter: 1, Choice: 0, Weight: 8}, Entry{Voter: 0, Choice: 0, Weight: 5}}, 2)
		expected := Report{YesWeight: 0, NoWeight: 19, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 2, Choice: 0, Weight: 0}, Entry{Voter: 1, Choice: 0, Weight: 3}, Entry{Voter: 1, Choice: 1, Weight: (-1)}, Entry{Voter: 2, Choice: 1, Weight: 0}, Entry{Voter: 1, Choice: 2, Weight: 2}}, 3)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 4, Choice: 3, Weight: (-2)}, Entry{Voter: 1, Choice: 3, Weight: 6}, Entry{Voter: 3, Choice: 0, Weight: 2}, Entry{Voter: 4, Choice: 1, Weight: (-2)}, Entry{Voter: 4, Choice: 3, Weight: (-2)}, Entry{Voter: 3, Choice: 0, Weight: 0}}, 4)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 1, Choice: 0, Weight: 5}, Entry{Voter: 3, Choice: 2, Weight: (-1)}, Entry{Voter: 3, Choice: 3, Weight: 5}, Entry{Voter: 0, Choice: 3, Weight: (-2)}, Entry{Voter: 4, Choice: 1, Weight: (-1)}, Entry{Voter: 1, Choice: 0, Weight: 3}, Entry{Voter: 2, Choice: 3, Weight: (-1)}}, 5)
		expected := Report{YesWeight: 0, NoWeight: 3, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 2, Choice: 3, Weight: (-1)}, Entry{Voter: 2, Choice: 0, Weight: (-1)}, Entry{Voter: 2, Choice: 2, Weight: 5}, Entry{Voter: 1, Choice: 3, Weight: 3}, Entry{Voter: 4, Choice: 2, Weight: 6}, Entry{Voter: 3, Choice: 0, Weight: 1}, Entry{Voter: 3, Choice: 0, Weight: 1}, Entry{Voter: 3, Choice: 0, Weight: 9}}, 6)
		expected := Report{YesWeight: 0, NoWeight: 9, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 0, Choice: 1, Weight: 5}, Entry{Voter: 0, Choice: 1, Weight: 0}}, 1)
		expected := Report{YesWeight: 0, NoWeight: 0, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 0, Choice: 1, Weight: 3}, Entry{Voter: 1, Choice: 0, Weight: 3}}, 3)
		expected := Report{YesWeight: 3, NoWeight: 3, QuorumMet: 0}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 0, Choice: 1, Weight: 3}}, 3)
		expected := Report{YesWeight: 3, NoWeight: 0, QuorumMet: 1}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Voter: 1, Choice: 1, Weight: 5}, Entry{Voter: 2, Choice: 0, Weight: 3}, Entry{Voter: 1, Choice: 2, Weight: 9}, Entry{Voter: 3, Choice: 1, Weight: 4}}, 4)
		expected := Report{YesWeight: 4, NoWeight: 3, QuorumMet: 1}
		if actual != expected {
			t.Fatalf("fixture 27: got %+v expected %+v", actual, expected)
		}
	}
}
