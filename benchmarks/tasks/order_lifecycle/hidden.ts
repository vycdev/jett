import { transition, type OrderEvent, type OrderState, type TransitionResult } from "./solution.js";

function check(state: OrderState["kind"], event: OrderEvent["kind"], expected: TransitionResult): void {
  const actual = transition({ kind: state }, { kind: event });
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`unexpected transition for ${state} + ${event}`);
  }
}

check("draft", "submit", { kind: "accepted", state: { kind: "submitted" } });
check("draft", "pay", { kind: "rejected", error: "event_not_allowed" });
check("draft", "ship", { kind: "rejected", error: "event_not_allowed" });
check("draft", "cancel", { kind: "accepted", state: { kind: "cancelled" } });
check("submitted", "submit", { kind: "rejected", error: "event_not_allowed" });
check("submitted", "pay", { kind: "accepted", state: { kind: "paid" } });
check("submitted", "ship", { kind: "rejected", error: "event_not_allowed" });
check("submitted", "cancel", { kind: "accepted", state: { kind: "cancelled" } });
check("paid", "submit", { kind: "rejected", error: "event_not_allowed" });
check("paid", "pay", { kind: "rejected", error: "event_not_allowed" });
check("paid", "ship", { kind: "accepted", state: { kind: "shipped" } });
check("paid", "cancel", { kind: "accepted", state: { kind: "cancelled" } });
check("shipped", "submit", { kind: "rejected", error: "terminal_state" });
check("shipped", "pay", { kind: "rejected", error: "terminal_state" });
check("shipped", "ship", { kind: "rejected", error: "terminal_state" });
check("shipped", "cancel", { kind: "rejected", error: "terminal_state" });
check("cancelled", "submit", { kind: "rejected", error: "terminal_state" });
check("cancelled", "pay", { kind: "rejected", error: "terminal_state" });
check("cancelled", "ship", { kind: "rejected", error: "terminal_state" });
check("cancelled", "cancel", { kind: "rejected", error: "terminal_state" });
