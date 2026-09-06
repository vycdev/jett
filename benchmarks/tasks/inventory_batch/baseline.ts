export type InventoryEvent =
  | { readonly kind: "stock"; readonly sku: string; readonly quantity: bigint }
  | { readonly kind: "sale"; readonly sku: string; readonly quantity: bigint };

export type InventoryResult =
  | { readonly kind: "accepted"; readonly balances: ReadonlyMap<string, bigint> }
  | { readonly kind: "rejected"; readonly index: bigint; readonly sku: string };

export function applyInventory(events: readonly InventoryEvent[]): InventoryResult {
  const balances = new Map<string, bigint>();
  for (const [index, event] of events.entries()) {
    if (event.quantity <= 0n) return { kind: "rejected", index: BigInt(index), sku: event.sku };
    const current = balances.get(event.sku) ?? 0n;
    switch (event.kind) {
      case "stock":
        balances.set(event.sku, current + event.quantity);
        break;
      case "sale":
        if (event.quantity > current) return { kind: "rejected", index: BigInt(index), sku: event.sku };
        balances.set(event.sku, current - event.quantity);
        break;
    }
  }
  return { kind: "accepted", balances };
}
