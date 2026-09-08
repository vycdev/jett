package benchmark

type InventoryEvent interface{ isInventoryEvent() }

type Stock struct {
	SKU      string
	Quantity int64
}
type Sale struct {
	SKU      string
	Quantity int64
}

func (Stock) isInventoryEvent() {}
func (Sale) isInventoryEvent()  {}

type InventoryResult interface{ isInventoryResult() }

type Accepted struct{ Balances map[string]int64 }
type Rejected struct {
	Index int64
	SKU   string
}

func (Accepted) isInventoryResult() {}
func (Rejected) isInventoryResult() {}

func ApplyInventory(events []InventoryEvent) InventoryResult {
	balances := make(map[string]int64)
	for index, event := range events {
		switch event := event.(type) {
		case Stock:
			if event.Quantity <= 0 {
				return Rejected{Index: int64(index), SKU: event.SKU}
			}
			balances[event.SKU] += event.Quantity
		case Sale:
			current := balances[event.SKU]
			if event.Quantity <= 0 || event.Quantity > current {
				return Rejected{Index: int64(index), SKU: event.SKU}
			}
			balances[event.SKU] = current - event.Quantity
		}
	}
	return Accepted{Balances: balances}
}
