from dataclasses import dataclass


@dataclass(frozen=True)
class Stock:
    sku: str
    quantity: int


@dataclass(frozen=True)
class Sale:
    sku: str
    quantity: int


type InventoryEvent = Stock | Sale


@dataclass(frozen=True)
class Accepted:
    balances: dict[str, int]


@dataclass(frozen=True)
class Rejected:
    index: int
    sku: str


type InventoryResult = Accepted | Rejected


def apply_inventory(events: list[InventoryEvent]) -> InventoryResult:
    balances: dict[str, int] = {}
    for index, event in enumerate(events):
        if event.quantity <= 0:
            return Rejected(index, event.sku)
        current = balances.get(event.sku, 0)
        if isinstance(event, Stock):
            balances[event.sku] = current + event.quantity
        else:
            if event.quantity > current:
                return Rejected(index, event.sku)
            balances[event.sku] = current - event.quantity
    return Accepted(balances)
