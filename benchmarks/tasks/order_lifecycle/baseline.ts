export type OrderState =
  | { readonly kind: "draft" }
  | { readonly kind: "submitted" }
  | { readonly kind: "paid" }
  | { readonly kind: "shipped" }
  | { readonly kind: "cancelled" };

export type OrderEvent =
  | { readonly kind: "submit" }
  | { readonly kind: "pay" }
  | { readonly kind: "ship" }
  | { readonly kind: "cancel" };

export type TransitionError = "event_not_allowed" | "terminal_state";

export type TransitionResult =
  | { readonly kind: "accepted"; readonly state: OrderState }
  | { readonly kind: "rejected"; readonly error: TransitionError };

function transitionFromDraft(event: OrderEvent): TransitionResult {
  switch (event.kind) {
    case "submit":
      return { kind: "accepted", state: { kind: "submitted" } };
    case "pay":
      return { kind: "rejected", error: "event_not_allowed" };
    case "ship":
      return { kind: "rejected", error: "event_not_allowed" };
    case "cancel":
      return { kind: "accepted", state: { kind: "cancelled" } };
  }
}

function transitionFromSubmitted(event: OrderEvent): TransitionResult {
  switch (event.kind) {
    case "submit":
      return { kind: "rejected", error: "event_not_allowed" };
    case "pay":
      return { kind: "accepted", state: { kind: "paid" } };
    case "ship":
      return { kind: "rejected", error: "event_not_allowed" };
    case "cancel":
      return { kind: "accepted", state: { kind: "cancelled" } };
  }
}

function transitionFromPaid(event: OrderEvent): TransitionResult {
  switch (event.kind) {
    case "submit":
      return { kind: "rejected", error: "event_not_allowed" };
    case "pay":
      return { kind: "rejected", error: "event_not_allowed" };
    case "ship":
      return { kind: "accepted", state: { kind: "shipped" } };
    case "cancel":
      return { kind: "accepted", state: { kind: "cancelled" } };
  }
}

function rejectTerminal(event: OrderEvent): TransitionResult {
  switch (event.kind) {
    case "submit":
      return { kind: "rejected", error: "terminal_state" };
    case "pay":
      return { kind: "rejected", error: "terminal_state" };
    case "ship":
      return { kind: "rejected", error: "terminal_state" };
    case "cancel":
      return { kind: "rejected", error: "terminal_state" };
  }
}

export function transition(state: OrderState, event: OrderEvent): TransitionResult {
  switch (state.kind) {
    case "draft":
      return transitionFromDraft(event);
    case "submitted":
      return transitionFromSubmitted(event);
    case "paid":
      return transitionFromPaid(event);
    case "shipped":
      return rejectTerminal(event);
    case "cancelled":
      return rejectTerminal(event);
  }
}
