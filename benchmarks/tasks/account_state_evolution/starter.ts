export type AccountState =
  | { readonly kind: "active" }
  | { readonly kind: "locked"; readonly failedAttempts: bigint }
  | { readonly kind: "closed" };

export function canSignIn(state: AccountState): boolean {
  switch (state.kind) {
    case "active":
      return true;
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
    case "locked":
      return `locked:${state.failedAttempts}`;
    case "closed":
      return "closed";
  }
}
