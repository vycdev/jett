import { canSignIn, statusLabel, suspend, type AccountState } from "./solution.js";

function checkState(actual: AccountState, expected: AccountState): void {
  const normalize = (value: AccountState): string =>
    JSON.stringify(value, (_key, field) =>
      typeof field === "bigint" ? field.toString() : field,
    );
  if (normalize(actual) !== normalize(expected)) throw new Error("unexpected state");
}

if (!canSignIn({ kind: "active" })) throw new Error("active must sign in");
if (canSignIn({ kind: "suspended", reason: "review" })) throw new Error("suspended signed in");
if (canSignIn({ kind: "locked", failedAttempts: 3n })) throw new Error("locked signed in");
if (canSignIn({ kind: "closed" })) throw new Error("closed signed in");
if (statusLabel({ kind: "active" }) !== "active") throw new Error("active label");
if (statusLabel({ kind: "suspended", reason: "review" }) !== "suspended:review") throw new Error("suspended label");
if (statusLabel({ kind: "locked", failedAttempts: 3n }) !== "locked:3") throw new Error("locked label");
if (statusLabel({ kind: "closed" }) !== "closed") throw new Error("closed label");
checkState(suspend({ kind: "active" }, "maintenance"), { kind: "suspended", reason: "maintenance" });
checkState(suspend({ kind: "suspended", reason: "review" }, "replacement"), { kind: "suspended", reason: "review" });
checkState(suspend({ kind: "locked", failedAttempts: 7n }, "ignored"), { kind: "locked", failedAttempts: 7n });
checkState(suspend({ kind: "closed" }, "ignored"), { kind: "closed" });
