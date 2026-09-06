package benchmark

import (
	"reflect"
	"testing"
)

func TestInventoryBatch(t *testing.T) {
	tests := []struct {
		events []InventoryEvent
		want   InventoryResult
	}{
		{[]InventoryEvent{}, Accepted{Balances: map[string]int64{}}},
		{[]InventoryEvent{Stock{SKU: "apple", Quantity: 5}, Sale{SKU: "apple", Quantity: 2}, Stock{SKU: "banana", Quantity: 4}}, Accepted{Balances: map[string]int64{"apple": 3, "banana": 4}}},
		{[]InventoryEvent{Stock{SKU: "apple", Quantity: 2}, Sale{SKU: "apple", Quantity: 2}}, Accepted{Balances: map[string]int64{"apple": 0}}},
		{[]InventoryEvent{Sale{SKU: "apple", Quantity: 1}}, Rejected{Index: 0, SKU: "apple"}},
		{[]InventoryEvent{Stock{SKU: "apple", Quantity: 2}, Sale{SKU: "apple", Quantity: 3}, Stock{SKU: "banana", Quantity: 9}}, Rejected{Index: 1, SKU: "apple"}},
		{[]InventoryEvent{Stock{SKU: "bad", Quantity: 0}}, Rejected{Index: 0, SKU: "bad"}},
	}
	for _, test := range tests {
		if got := ApplyInventory(test.events); !reflect.DeepEqual(got, test.want) {
			t.Fatalf("got %#v, want %#v", got, test.want)
		}
	}
}
