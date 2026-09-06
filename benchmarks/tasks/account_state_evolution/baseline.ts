export type AccountState =
  | { readonly kind: "active" }
  | { readonly kind: "suspended"; readonly reason: string }
  | { readonly kind: "locked"; readonly failedAttempts: bigint }
  | { readonly kind: "closed" };

export function canSignIn(state: AccountState): boolean {
  switch (state.kind) {
    case "active":
      return true;
    case "suspended":
      return false;
    case "locked":
      return false;
    case "closed":
      return false;
  }
}

export function statusLabel(state: AccountState): string {
  switch (state.kind) {
    case "active":
      return "active";
    case "suspended":
      return `suspended:${state.reason}`;
    case "locked":
      return `locked:${state.failedAttempts}`;
    case "closed":
      return "closed";
  }
}

export function suspend(state: AccountState, reason: string): AccountState {
  switch (state.kind) {
    case "active":
      return { kind: "suspended", reason };
    case "suspended":
      return state;
    case "locked":
      return state;
    case "closed":
      return state;
  }
}
