export type ScoreError = "malformed" | "invalid_score" | "duplicate_name";

export type ScoreResult =
  | { readonly kind: "accepted"; readonly scores: ReadonlyMap<string, bigint> }
  | { readonly kind: "rejected"; readonly line: bigint; readonly error: ScoreError };

const MIN_INT64 = -(2n ** 63n);
const MAX_INT64 = 2n ** 63n - 1n;

export function parseScores(lines: readonly string[]): ScoreResult {
  const scores = new Map<string, bigint>();
  for (let line = 0; line < lines.length; line++) {
    const parts = lines[line]?.split("=") ?? [];
    const name = parts[0];
    const rawScore = parts[1];
    if (parts.length !== 2 || name === undefined || name === "" || rawScore === undefined) {
      return { kind: "rejected", line: BigInt(line), error: "malformed" };
    }
    let score: bigint;
    try {
      score = BigInt(rawScore);
    } catch {
      return { kind: "rejected", line: BigInt(line), error: "invalid_score" };
    }
    if (score < MIN_INT64 || score > MAX_INT64 || score.toString() !== rawScore) {
      return { kind: "rejected", line: BigInt(line), error: "invalid_score" };
    }
    if (scores.has(name)) return { kind: "rejected", line: BigInt(line), error: "duplicate_name" };
    scores.set(name, score);
  }
  return { kind: "accepted", scores };
}
