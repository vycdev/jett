package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Allowed: 0, Denied: 0, Defaulted: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Allowed: 0, Denied: 5, Defaulted: 5}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Allowed: 0, Denied: 1, Defaulted: 1}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 3, Allowed: 1, Specificity: 1}}, 2)
		expected := Report{Allowed: 0, Denied: 2, Defaulted: 2}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 4, Allowed: 1, Specificity: 3}, Entry{Principal: 4, Allowed: 1, Specificity: 0}}, 3)
		expected := Report{Allowed: 0, Denied: 3, Defaulted: 3}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 0, Allowed: 0, Specificity: 0}, Entry{Principal: 0, Allowed: 1, Specificity: 1}, Entry{Principal: 2, Allowed: 1, Specificity: 2}}, 4)
		expected := Report{Allowed: 2, Denied: 2, Defaulted: 2}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 4, Allowed: 0, Specificity: 3}, Entry{Principal: 0, Allowed: 0, Specificity: 3}, Entry{Principal: 2, Allowed: 0, Specificity: 2}, Entry{Principal: 1, Allowed: 1, Specificity: 0}}, 5)
		expected := Report{Allowed: 1, Denied: 4, Defaulted: 1}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 1, Allowed: 1, Specificity: 0}, Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 4, Allowed: 1, Specificity: 2}, Entry{Principal: 3, Allowed: 0, Specificity: 3}, Entry{Principal: 4, Allowed: 0, Specificity: 0}}, 6)
		expected := Report{Allowed: 2, Denied: 4, Defaulted: 3}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 1, Allowed: 1, Specificity: 0}, Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 0, Allowed: 1, Specificity: 3}, Entry{Principal: 3, Allowed: 0, Specificity: 3}, Entry{Principal: 1, Allowed: 1, Specificity: 0}, Entry{Principal: 0, Allowed: 1, Specificity: 1}}, 7)
		expected := Report{Allowed: 2, Denied: 5, Defaulted: 4}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 3, Allowed: 0, Specificity: 2}, Entry{Principal: 4, Allowed: 0, Specificity: 3}, Entry{Principal: 0, Allowed: 1, Specificity: 2}, Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 2, Allowed: 0, Specificity: 2}, Entry{Principal: 1, Allowed: 1, Specificity: 1}}, 8)
		expected := Report{Allowed: 2, Denied: 6, Defaulted: 3}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 3, Allowed: 1, Specificity: 1}, Entry{Principal: 2, Allowed: 1, Specificity: 2}, Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 2, Allowed: 0, Specificity: 3}, Entry{Principal: 0, Allowed: 1, Specificity: 2}, Entry{Principal: 1, Allowed: 0, Specificity: 0}, Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 1, Allowed: 0, Specificity: 0}}, 1)
		expected := Report{Allowed: 1, Denied: 0, Defaulted: 0}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 1, Allowed: 1, Specificity: 3}, Entry{Principal: 4, Allowed: 1, Specificity: 0}, Entry{Principal: 2, Allowed: 1, Specificity: 2}, Entry{Principal: 2, Allowed: 0, Specificity: 1}, Entry{Principal: 0, Allowed: 1, Specificity: 1}, Entry{Principal: 2, Allowed: 1, Specificity: 3}, Entry{Principal: 3, Allowed: 0, Specificity: 1}, Entry{Principal: 3, Allowed: 1, Specificity: 1}, Entry{Principal: 0, Allowed: 1, Specificity: 0}}, 2)
		expected := Report{Allowed: 2, Denied: 0, Defaulted: 0}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 4, Allowed: 1, Specificity: 1}, Entry{Principal: 1, Allowed: 0, Specificity: 1}, Entry{Principal: 1, Allowed: 1, Specificity: 1}, Entry{Principal: 1, Allowed: 0, Specificity: 3}, Entry{Principal: 3, Allowed: 1, Specificity: 2}, Entry{Principal: 2, Allowed: 1, Specificity: 1}, Entry{Principal: 1, Allowed: 0, Specificity: 2}, Entry{Principal: 1, Allowed: 0, Specificity: 2}, Entry{Principal: 1, Allowed: 0, Specificity: 0}, Entry{Principal: 1, Allowed: 0, Specificity: 3}}, 3)
		expected := Report{Allowed: 1, Denied: 2, Defaulted: 1}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 3, Allowed: 0, Specificity: 3}, Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 3, Allowed: 0, Specificity: 3}, Entry{Principal: 3, Allowed: 1, Specificity: 0}, Entry{Principal: 1, Allowed: 1, Specificity: 0}, Entry{Principal: 3, Allowed: 1, Specificity: 1}, Entry{Principal: 2, Allowed: 0, Specificity: 1}, Entry{Principal: 3, Allowed: 1, Specificity: 2}, Entry{Principal: 0, Allowed: 1, Specificity: 1}, Entry{Principal: 1, Allowed: 1, Specificity: 2}, Entry{Principal: 4, Allowed: 1, Specificity: 0}}, 4)
		expected := Report{Allowed: 2, Denied: 2, Defaulted: 0}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 2, Allowed: 1, Specificity: 1}, Entry{Principal: 0, Allowed: 0, Specificity: 3}, Entry{Principal: 3, Allowed: 1, Specificity: 2}, Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 0, Allowed: 1, Specificity: 3}, Entry{Principal: 0, Allowed: 0, Specificity: 3}, Entry{Principal: 2, Allowed: 1, Specificity: 0}, Entry{Principal: 4, Allowed: 0, Specificity: 0}, Entry{Principal: 1, Allowed: 0, Specificity: 1}, Entry{Principal: 1, Allowed: 1, Specificity: 0}, Entry{Principal: 4, Allowed: 0, Specificity: 3}, Entry{Principal: 4, Allowed: 0, Specificity: 1}}, 5)
		expected := Report{Allowed: 2, Denied: 3, Defaulted: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Allowed: 0, Denied: 6, Defaulted: 6}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 0, Allowed: 0, Specificity: 0}}, 7)
		expected := Report{Allowed: 0, Denied: 7, Defaulted: 6}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 3, Allowed: 1, Specificity: 0}, Entry{Principal: 1, Allowed: 0, Specificity: 0}}, 8)
		expected := Report{Allowed: 1, Denied: 7, Defaulted: 6}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 2, Allowed: 0, Specificity: 1}, Entry{Principal: 0, Allowed: 1, Specificity: 1}, Entry{Principal: 1, Allowed: 0, Specificity: 2}}, 1)
		expected := Report{Allowed: 1, Denied: 0, Defaulted: 0}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 2, Allowed: 1, Specificity: 0}, Entry{Principal: 1, Allowed: 1, Specificity: 3}, Entry{Principal: 0, Allowed: 1, Specificity: 1}, Entry{Principal: 0, Allowed: 1, Specificity: 0}}, 2)
		expected := Report{Allowed: 2, Denied: 0, Defaulted: 0}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 3, Allowed: 0, Specificity: 1}, Entry{Principal: 1, Allowed: 0, Specificity: 3}, Entry{Principal: 3, Allowed: 1, Specificity: 0}, Entry{Principal: 3, Allowed: 1, Specificity: 3}, Entry{Principal: 0, Allowed: 1, Specificity: 0}}, 3)
		expected := Report{Allowed: 1, Denied: 2, Defaulted: 1}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 4, Allowed: 0, Specificity: 0}, Entry{Principal: 1, Allowed: 0, Specificity: 2}, Entry{Principal: 2, Allowed: 1, Specificity: 0}, Entry{Principal: 2, Allowed: 1, Specificity: 0}, Entry{Principal: 2, Allowed: 0, Specificity: 0}, Entry{Principal: 2, Allowed: 1, Specificity: 3}}, 4)
		expected := Report{Allowed: 1, Denied: 3, Defaulted: 2}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 1, Allowed: 1, Specificity: 2}, Entry{Principal: 4, Allowed: 1, Specificity: 3}, Entry{Principal: 0, Allowed: 0, Specificity: 3}, Entry{Principal: 0, Allowed: 0, Specificity: 3}, Entry{Principal: 0, Allowed: 1, Specificity: 1}, Entry{Principal: 1, Allowed: 0, Specificity: 2}, Entry{Principal: 2, Allowed: 0, Specificity: 3}}, 5)
		expected := Report{Allowed: 1, Denied: 4, Defaulted: 1}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 3, Allowed: 0, Specificity: 0}, Entry{Principal: 4, Allowed: 1, Specificity: 2}, Entry{Principal: 2, Allowed: 1, Specificity: 0}, Entry{Principal: 4, Allowed: 1, Specificity: 0}, Entry{Principal: 3, Allowed: 0, Specificity: 2}, Entry{Principal: 4, Allowed: 1, Specificity: 0}, Entry{Principal: 4, Allowed: 1, Specificity: 0}, Entry{Principal: 4, Allowed: 0, Specificity: 2}}, 6)
		expected := Report{Allowed: 1, Denied: 5, Defaulted: 3}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 0, Allowed: 0, Specificity: 1}, Entry{Principal: 0, Allowed: 1, Specificity: 1}}, 1)
		expected := Report{Allowed: 1, Denied: 0, Defaulted: 0}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 0, Allowed: 1, Specificity: 2}, Entry{Principal: 0, Allowed: 0, Specificity: 1}, Entry{Principal: 4, Allowed: 1, Specificity: 8}}, 2)
		expected := Report{Allowed: 1, Denied: 1, Defaulted: 1}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Principal: 0, Allowed: 1, Specificity: 2}, Entry{Principal: 0, Allowed: 0, Specificity: 1}, Entry{Principal: 1, Allowed: 1, Specificity: 3}, Entry{Principal: 1, Allowed: 0, Specificity: 3}}, 3)
		expected := Report{Allowed: 1, Denied: 2, Defaulted: 1}
		if actual != expected {
			t.Fatalf("fixture 26: got %+v expected %+v", actual, expected)
		}
	}
}
