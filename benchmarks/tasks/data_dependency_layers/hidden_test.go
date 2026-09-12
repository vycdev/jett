package benchmark

import "testing"

func TestFixtures(t *testing.T) {
	{
		actual := solve([]Entry{}, 0)
		expected := Report{Layers: 0, LastLayerCount: 0, LayerSum: 0}
		if actual != expected {
			t.Fatalf("fixture 0: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 5)
		expected := Report{Layers: 1, LastLayerCount: 5, LayerSum: 0}
		if actual != expected {
			t.Fatalf("fixture 1: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 1)
		expected := Report{Layers: 1, LastLayerCount: 1, LayerSum: 0}
		if actual != expected {
			t.Fatalf("fixture 2: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 1}}, 2)
		expected := Report{Layers: 2, LastLayerCount: 1, LayerSum: 1}
		if actual != expected {
			t.Fatalf("fixture 3: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 2}}, 3)
		expected := Report{Layers: 2, LastLayerCount: 1, LayerSum: 1}
		if actual != expected {
			t.Fatalf("fixture 4: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 2}}, 4)
		expected := Report{Layers: 2, LastLayerCount: 2, LayerSum: 2}
		if actual != expected {
			t.Fatalf("fixture 5: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 2, Dependent: 3}}, 5)
		expected := Report{Layers: 3, LastLayerCount: 1, LayerSum: 3}
		if actual != expected {
			t.Fatalf("fixture 6: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 0, Dependent: 4}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 4, Dependent: 5}, Entry{Prerequisite: 4, Dependent: 5}}, 6)
		expected := Report{Layers: 3, LastLayerCount: 1, LayerSum: 4}
		if actual != expected {
			t.Fatalf("fixture 7: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 6}, Entry{Prerequisite: 4, Dependent: 5}, Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 0, Dependent: 4}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 3, Dependent: 5}}, 7)
		expected := Report{Layers: 3, LastLayerCount: 1, LayerSum: 6}
		if actual != expected {
			t.Fatalf("fixture 8: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 3, Dependent: 4}, Entry{Prerequisite: 3, Dependent: 5}, Entry{Prerequisite: 6, Dependent: 7}, Entry{Prerequisite: 2, Dependent: 4}, Entry{Prerequisite: 3, Dependent: 5}, Entry{Prerequisite: 6, Dependent: 7}, Entry{Prerequisite: 3, Dependent: 4}}, 8)
		expected := Report{Layers: 2, LastLayerCount: 3, LayerSum: 3}
		if actual != expected {
			t.Fatalf("fixture 9: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}}, 2)
		expected := Report{Layers: 2, LastLayerCount: 1, LayerSum: 1}
		if actual != expected {
			t.Fatalf("fixture 10: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 0, Dependent: 1}}, 3)
		expected := Report{Layers: 3, LastLayerCount: 1, LayerSum: 3}
		if actual != expected {
			t.Fatalf("fixture 11: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 3}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 2}}, 4)
		expected := Report{Layers: 4, LastLayerCount: 1, LayerSum: 6}
		if actual != expected {
			t.Fatalf("fixture 12: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 1, Dependent: 4}, Entry{Prerequisite: 3, Dependent: 4}, Entry{Prerequisite: 2, Dependent: 4}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 4}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 3, Dependent: 4}, Entry{Prerequisite: 3, Dependent: 4}, Entry{Prerequisite: 0, Dependent: 4}, Entry{Prerequisite: 3, Dependent: 4}}, 5)
		expected := Report{Layers: 4, LastLayerCount: 1, LayerSum: 6}
		if actual != expected {
			t.Fatalf("fixture 13: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{}, 6)
		expected := Report{Layers: 1, LastLayerCount: 6, LayerSum: 0}
		if actual != expected {
			t.Fatalf("fixture 14: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 2, Dependent: 3}}, 7)
		expected := Report{Layers: 2, LastLayerCount: 1, LayerSum: 1}
		if actual != expected {
			t.Fatalf("fixture 15: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 3, Dependent: 4}, Entry{Prerequisite: 3, Dependent: 5}}, 8)
		expected := Report{Layers: 2, LastLayerCount: 2, LayerSum: 2}
		if actual != expected {
			t.Fatalf("fixture 16: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 1}}, 2)
		expected := Report{Layers: 2, LastLayerCount: 1, LayerSum: 1}
		if actual != expected {
			t.Fatalf("fixture 17: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 2}}, 3)
		expected := Report{Layers: 2, LastLayerCount: 1, LayerSum: 1}
		if actual != expected {
			t.Fatalf("fixture 18: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 3}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 2, Dependent: 3}}, 4)
		expected := Report{Layers: 4, LastLayerCount: 1, LayerSum: 6}
		if actual != expected {
			t.Fatalf("fixture 19: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 4}}, 5)
		expected := Report{Layers: 4, LastLayerCount: 1, LayerSum: 8}
		if actual != expected {
			t.Fatalf("fixture 20: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 0, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 3}, Entry{Prerequisite: 2, Dependent: 5}, Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 3, Dependent: 5}, Entry{Prerequisite: 2, Dependent: 5}, Entry{Prerequisite: 0, Dependent: 5}}, 6)
		expected := Report{Layers: 4, LastLayerCount: 1, LayerSum: 6}
		if actual != expected {
			t.Fatalf("fixture 21: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 3, Dependent: 4}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 0, Dependent: 1}, Entry{Prerequisite: 0, Dependent: 4}}, 6)
		expected := Report{Layers: 5, LastLayerCount: 1, LayerSum: 10}
		if actual != expected {
			t.Fatalf("fixture 22: got %+v expected %+v", actual, expected)
		}
	}
	{
		actual := solve([]Entry{Entry{Prerequisite: 0, Dependent: 2}, Entry{Prerequisite: 1, Dependent: 2}, Entry{Prerequisite: 2, Dependent: 3}, Entry{Prerequisite: 1, Dependent: 4}}, 5)
		expected := Report{Layers: 3, LastLayerCount: 1, LayerSum: 4}
		if actual != expected {
			t.Fatalf("fixture 23: got %+v expected %+v", actual, expected)
		}
	}
}
