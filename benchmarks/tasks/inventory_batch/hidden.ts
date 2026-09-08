import { applyInventory, type InventoryEvent, type InventoryResult } from "./solution.js";

function check(events: readonly InventoryEvent[], expected: InventoryResult): void {
  const actual = applyInventory(events);
  if (actual.kind !== expected.kind) throw new Error("unexpected result kind");
  if (actual.kind === "rejected" && expected.kind === "rejected") {
    if (actual.index !== expected.index || actual.sku !== expected.sku) throw new Error("unexpected rejection");
  }
  if (actual.kind === "accepted" && expected.kind === "accepted") {
    if (actual.balances.size !== expected.balances.size) throw new Error("unexpected map size");
    for (const [sku, quantity] of expected.balances) {
      if (actual.balances.get(sku) !== quantity) throw new Error("unexpected balance");
    }
  }
}

check([], { kind: "accepted", balances: new Map() });
check([{ kind: "stock", sku: "apple", quantity: 5n }, { kind: "sale", sku: "apple", quantity: 2n }, { kind: "stock", sku: "banana", quantity: 4n }], { kind: "accepted", balances: new Map([["apple", 3n], ["banana", 4n]]) });
check([{ kind: "stock", sku: "apple", quantity: 2n }, { kind: "sale", sku: "apple", quantity: 2n }], { kind: "accepted", balances: new Map([["apple", 0n]]) });
check([{ kind: "sale", sku: "apple", quantity: 1n }], { kind: "rejected", index: 0n, sku: "apple" });
check([{ kind: "stock", sku: "apple", quantity: 2n }, { kind: "sale", sku: "apple", quantity: 3n }, { kind: "stock", sku: "banana", quantity: 9n }], { kind: "rejected", index: 1n, sku: "apple" });
check([{ kind: "stock", sku: "bad", quantity: 0n }], { kind: "rejected", index: 0n, sku: "bad" });
