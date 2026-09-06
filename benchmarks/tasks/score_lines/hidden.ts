import { parseScores, type ScoreError, type ScoreResult } from "./solution.js";

function parsed(entries: readonly (readonly [string, bigint])[]): ScoreResult {
  return { kind: "accepted", scores: new Map(entries) };
}

function failed(line: bigint, error: ScoreError): ScoreResult {
  return { kind: "rejected", line, error };
}

function check(lines: readonly string[], expected: ScoreResult): void {
  const actual = parseScores(lines);
  if (actual.kind !== expected.kind) throw new Error("unexpected result kind");
  if (actual.kind === "rejected" && expected.kind === "rejected") {
    if (actual.line !== expected.line || actual.error !== expected.error) throw new Error("unexpected failure");
  }
  if (actual.kind === "accepted" && expected.kind === "accepted") {
    if (actual.scores.size !== expected.scores.size) throw new Error("unexpected map size");
    for (const [name, score] of expected.scores) {
      if (actual.scores.get(name) !== score) throw new Error("unexpected score");
    }
  }
}

check([], parsed([]));
check(["ada=10", "bob=-2"], parsed([["ada", 10n], ["bob", -2n]]));
check(["ada=9223372036854775807", "bob=-9223372036854775808"], parsed([["ada", 9223372036854775807n], ["bob", -9223372036854775808n]]));
check(["ada=1", "ada=2"], failed(1n, "duplicate_name"));
check(["missing"], failed(0n, "malformed"));
check(["=3"], failed(0n, "malformed"));
check(["a=1=2"], failed(0n, "malformed"));
check(["ada=01"], failed(0n, "invalid_score"));
check(["ada=-0"], failed(0n, "invalid_score"));
check(["ada=9223372036854775808"], failed(0n, "invalid_score"));
check(["ada=1", "ada=bad"], failed(1n, "invalid_score"));
