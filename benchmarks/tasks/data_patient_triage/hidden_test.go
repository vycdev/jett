package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Selected: 0, PatientChecksum: 0, SeveritySum: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Selected: 0, PatientChecksum: 0, SeveritySum: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Selected: 0, PatientChecksum: 0, SeveritySum: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 3, Severity: 8, Arrival: 3}}, 2)
		expected := Report{Selected: 1, PatientChecksum: 4, SeveritySum: 8}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 1, Severity: 6, Arrival: 3}, Entry{Patient: 3, Severity: 7, Arrival: 5}}, 3)
		expected := Report{Selected: 2, PatientChecksum: 8, SeveritySum: 13}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 2, Severity: 11, Arrival: 1}, Entry{Patient: 0, Severity: 0, Arrival: 6}, Entry{Patient: 0, Severity: (-2), Arrival: 5}}, 4)
		expected := Report{Selected: 3, PatientChecksum: 8, SeveritySum: 9}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 3, Severity: 10, Arrival: 2}, Entry{Patient: 2, Severity: 2, Arrival: 3}, Entry{Patient: 4, Severity: 0, Arrival: 4}, Entry{Patient: 0, Severity: 11, Arrival: 2}}, 5)
		expected := Report{Selected: 4, PatientChecksum: 38, SeveritySum: 23}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 3, Severity: 10, Arrival: 3}, Entry{Patient: 0, Severity: 5, Arrival: 5}, Entry{Patient: 4, Severity: 2, Arrival: 6}, Entry{Patient: 1, Severity: 3, Arrival: 1}, Entry{Patient: 1, Severity: 1, Arrival: 1}}, 6)
		expected := Report{Selected: 5, PatientChecksum: 42, SeveritySum: 21}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 3, Severity: 5, Arrival: 6}, Entry{Patient: 1, Severity: (-2), Arrival: 5}, Entry{Patient: 3, Severity: 11, Arrival: 6}, Entry{Patient: 4, Severity: 2, Arrival: 4}, Entry{Patient: 0, Severity: 10, Arrival: 6}, Entry{Patient: 3, Severity: 6, Arrival: 5}}, 7)
		expected := Report{Selected: 6, PatientChecksum: 71, SeveritySum: 32}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 1, Severity: 5, Arrival: 1}, Entry{Patient: 1, Severity: 1, Arrival: 1}, Entry{Patient: 3, Severity: (-1), Arrival: 1}, Entry{Patient: 0, Severity: 8, Arrival: 4}, Entry{Patient: 3, Severity: 7, Arrival: 4}, Entry{Patient: 0, Severity: 5, Arrival: 4}, Entry{Patient: 1, Severity: 11, Arrival: 3}}, 8)
		expected := Report{Selected: 7, PatientChecksum: 69, SeveritySum: 36}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 0, Severity: (-3), Arrival: 3}, Entry{Patient: 1, Severity: 7, Arrival: 4}, Entry{Patient: 1, Severity: 7, Arrival: 1}, Entry{Patient: 3, Severity: (-2), Arrival: 3}, Entry{Patient: 4, Severity: (-1), Arrival: 4}, Entry{Patient: 0, Severity: 4, Arrival: 5}, Entry{Patient: 2, Severity: 8, Arrival: 4}, Entry{Patient: 1, Severity: (-2), Arrival: 6}}, 1)
		expected := Report{Selected: 1, PatientChecksum: 3, SeveritySum: 8}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 2, Severity: 10, Arrival: 2}, Entry{Patient: 2, Severity: 11, Arrival: 2}, Entry{Patient: 4, Severity: 2, Arrival: 2}, Entry{Patient: 3, Severity: 1, Arrival: 2}, Entry{Patient: 2, Severity: 1, Arrival: 5}, Entry{Patient: 2, Severity: 4, Arrival: 2}, Entry{Patient: 0, Severity: 2, Arrival: 1}, Entry{Patient: 3, Severity: (-3), Arrival: 4}, Entry{Patient: 4, Severity: 2, Arrival: 2}}, 2)
		expected := Report{Selected: 2, PatientChecksum: 9, SeveritySum: 21}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 0, Severity: 7, Arrival: 1}, Entry{Patient: 3, Severity: (-3), Arrival: 1}, Entry{Patient: 1, Severity: 5, Arrival: 6}, Entry{Patient: 0, Severity: 10, Arrival: 1}, Entry{Patient: 1, Severity: 8, Arrival: 3}, Entry{Patient: 3, Severity: 6, Arrival: 3}, Entry{Patient: 0, Severity: 2, Arrival: 3}, Entry{Patient: 4, Severity: 5, Arrival: 3}, Entry{Patient: 2, Severity: (-1), Arrival: 2}, Entry{Patient: 0, Severity: 1, Arrival: 2}}, 3)
		expected := Report{Selected: 3, PatientChecksum: 8, SeveritySum: 25}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 2, Severity: 5, Arrival: 3}, Entry{Patient: 4, Severity: 3, Arrival: 6}, Entry{Patient: 3, Severity: 11, Arrival: 1}, Entry{Patient: 1, Severity: 8, Arrival: 6}, Entry{Patient: 3, Severity: 4, Arrival: 2}, Entry{Patient: 0, Severity: 1, Arrival: 1}, Entry{Patient: 4, Severity: 3, Arrival: 2}, Entry{Patient: 1, Severity: 5, Arrival: 6}, Entry{Patient: 4, Severity: 6, Arrival: 5}, Entry{Patient: 1, Severity: 9, Arrival: 5}, Entry{Patient: 4, Severity: 7, Arrival: 2}}, 4)
		expected := Report{Selected: 4, PatientChecksum: 34, SeveritySum: 35}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 1, Severity: 3, Arrival: 2}, Entry{Patient: 1, Severity: 7, Arrival: 2}, Entry{Patient: 3, Severity: 4, Arrival: 6}, Entry{Patient: 2, Severity: 6, Arrival: 3}, Entry{Patient: 2, Severity: 11, Arrival: 3}, Entry{Patient: 1, Severity: 0, Arrival: 5}, Entry{Patient: 0, Severity: 2, Arrival: 2}, Entry{Patient: 1, Severity: 10, Arrival: 3}, Entry{Patient: 1, Severity: (-1), Arrival: 5}, Entry{Patient: 0, Severity: (-1), Arrival: 1}, Entry{Patient: 3, Severity: 11, Arrival: 6}, Entry{Patient: 3, Severity: (-3), Arrival: 4}}, 5)
		expected := Report{Selected: 5, PatientChecksum: 40, SeveritySum: 45}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Selected: 0, PatientChecksum: 0, SeveritySum: 0}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 3, Severity: (-1), Arrival: 5}}, 7)
		expected := Report{Selected: 1, PatientChecksum: 4, SeveritySum: (-1)}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 0, Severity: 4, Arrival: 1}, Entry{Patient: 4, Severity: 8, Arrival: 4}}, 8)
		expected := Report{Selected: 2, PatientChecksum: 7, SeveritySum: 12}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 3, Severity: 7, Arrival: 3}, Entry{Patient: 0, Severity: (-1), Arrival: 4}, Entry{Patient: 0, Severity: 3, Arrival: 4}}, 1)
		expected := Report{Selected: 1, PatientChecksum: 4, SeveritySum: 7}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 1, Severity: 11, Arrival: 3}, Entry{Patient: 0, Severity: 5, Arrival: 2}, Entry{Patient: 3, Severity: 4, Arrival: 3}, Entry{Patient: 0, Severity: 4, Arrival: 2}}, 2)
		expected := Report{Selected: 2, PatientChecksum: 4, SeveritySum: 16}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 1, Severity: 1, Arrival: 5}, Entry{Patient: 2, Severity: 5, Arrival: 4}, Entry{Patient: 4, Severity: (-3), Arrival: 3}, Entry{Patient: 2, Severity: 5, Arrival: 5}, Entry{Patient: 1, Severity: (-3), Arrival: 6}}, 3)
		expected := Report{Selected: 3, PatientChecksum: 15, SeveritySum: 11}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 0, Severity: 4, Arrival: 4}, Entry{Patient: 2, Severity: 1, Arrival: 6}, Entry{Patient: 3, Severity: 10, Arrival: 1}, Entry{Patient: 4, Severity: (-3), Arrival: 1}, Entry{Patient: 2, Severity: 4, Arrival: 1}, Entry{Patient: 1, Severity: 4, Arrival: 6}}, 4)
		expected := Report{Selected: 4, PatientChecksum: 21, SeveritySum: 22}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 2, Severity: 1, Arrival: 6}, Entry{Patient: 0, Severity: 6, Arrival: 5}, Entry{Patient: 1, Severity: 6, Arrival: 5}, Entry{Patient: 0, Severity: 0, Arrival: 5}, Entry{Patient: 1, Severity: 0, Arrival: 2}, Entry{Patient: 2, Severity: 11, Arrival: 1}, Entry{Patient: 4, Severity: (-1), Arrival: 4}}, 5)
		expected := Report{Selected: 5, PatientChecksum: 33, SeveritySum: 24}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 4, Severity: 6, Arrival: 1}, Entry{Patient: 4, Severity: (-1), Arrival: 1}, Entry{Patient: 0, Severity: (-3), Arrival: 4}, Entry{Patient: 2, Severity: 9, Arrival: 5}, Entry{Patient: 0, Severity: (-1), Arrival: 2}, Entry{Patient: 0, Severity: 2, Arrival: 2}, Entry{Patient: 1, Severity: (-2), Arrival: 3}, Entry{Patient: 1, Severity: (-1), Arrival: 6}}, 6)
		expected := Report{Selected: 6, PatientChecksum: 53, SeveritySum: 14}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 8, Severity: 5, Arrival: 2}, Entry{Patient: 1, Severity: 5, Arrival: 2}, Entry{Patient: 3, Severity: 5, Arrival: 1}}, 2)
		expected := Report{Selected: 2, PatientChecksum: 22, SeveritySum: 10}
		if actual != expected {
			t.Fatalf("fixture 24: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Patient: 9, Severity: 2, Arrival: 1}, Entry{Patient: 3, Severity: 5, Arrival: 4}, Entry{Patient: 4, Severity: 5, Arrival: 4}, Entry{Patient: 8, Severity: 5, Arrival: 2}}, 2)
		expected := Report{Selected: 2, PatientChecksum: 17, SeveritySum: 10}
		if actual != expected {
			t.Fatalf("fixture 25: got %+v expected %+v", actual, expected)
		}
	}
}
