# Rust language and tooling reference

Use `Cargo.toml`, the crate edition, enabled features, and nearby accepted code as authority.

## Type model

- Model closed alternatives with enums and keep payloads on the variant that owns them.
- Use structs for records, `Option<T>` for absence, and `Result<T, E>` for fallible operations.
- Match enums exhaustively. Avoid wildcard arms when visibility of future variants matters.
- Give ambiguous collections or constructors the smallest useful explicit type.
- Keep conversions checked at boundaries. Avoid `as`, `unsafe`, or trait objects merely to silence a mismatch.

## Ownership and borrowing

Choose ownership from the interface:

- Accept `&T` to observe, `&mut T` to mutate in place, and `T` to consume or store.
- Return owned values when the result must outlive borrowed input.
- Clone only when independent ownership is required and the cost is justified.
- Use iterators without collecting when no owned intermediate collection is needed.

For recursive values, introduce indirection such as `Box<T>` only at the recursive boundary that requires it. Let lifetime elision handle ordinary cases; add named lifetimes when relationships are otherwise ambiguous.

## Failure and control flow

Use `?` when the function preserves or deliberately converts the error. Match when recovery depends on the exact case. Reserve `panic!`, `unwrap`, and `expect` for impossible states proven by a local invariant or for tests, following project policy.

Use exhaustive `match`, `if let`, or `let else` according to how many cases matter. Keep borrow scopes small instead of cloning around a borrow-checker error.

## Verification loop

Run focused commands before workspace-wide checks:

```text
cargo fmt --check
cargo check -p crate_name
cargo clippy -p crate_name --all-targets --all-features -- -D warnings
cargo test -p crate_name focused_test
cargo test
```

Follow repository feature flags and lint policy. Repair the first causal compiler diagnostic; later borrow or trait errors are often consequences.

## Provenance

This reference follows the Rust 2024 language and standard Cargo tooling. It intentionally contains no evaluation task, solution, hidden case, or failure-specific recipe.
