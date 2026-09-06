# Jett benchmark subset v0.4

This capability profile extends `jett-v0.3` with stable collection, struct,
optional, and parsing features. Earlier task behavior remains unchanged.

## Added to v0.4

- user-defined structs and field access;
- `set[T]` construction, membership, and insertion;
- `map[K, V]` construction, lookup, membership, and insertion;
- explicit `clone` when a move-only collection must be retained;
- `optional[T]` with `some` and `none`;
- handled `result[T, E]` values and typed failures;
- string splitting and canonical `int64` parsing.

The four tasks are `first_duplicate`, `merge_sorted_intervals`,
`inventory_batch`, and `score_lines`. Together they cover order-sensitive set
use, list-to-list struct transformation, typed event accumulation into a map,
and structured parsing failures.

## Submission contract

The v0.3 contract still applies. Compiler-shipped collection and string
modules are available to benchmark submissions. Maps, sets, and lists are
move-only; consuming updates return the new collection, and `view` or explicit
`clone` must be used where the source must remain available.

Any change to collection ownership, optional/result handling, parsing
semantics, public declarations, policy, or hidden grading increments the
affected version.
