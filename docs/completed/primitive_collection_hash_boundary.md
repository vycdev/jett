# Primitive Collection Hashing Boundary

Status: completed. Map keys and set elements remain primitive-only until Jett
has real pressure for a custom hashing abstraction.

## Decision

Jett does not expose `Hashable`, a raw hash integer, hash bytes, or an opaque
hasher state. Collection type formation accepts only:

- signed and unsigned integer primitives;
- `string`;
- `bool`;
- refinements whose ultimate base is one of the above.

The restriction applies to `map[K, V]` keys and `set[T]` elements. Values in a
map remain unrestricted. Floats, secrets, enums, lists, maps, sets, optionals,
results, functions, interfaces, machines, actors, and user aggregates are not
valid hash keys/elements. E0359 reports the unsupported type at its collection
type argument.

Structured data uses explicit primitive identity:

```jett
map[string, User] users_by_id
set[string] selected_user_ids
```

This avoids exposing an arbitrary `uint64` contract, inventing an opaque hash
state, or accepting structural hashing that can silently change when fields
change. It also avoids an unchecked requirement that custom equality and
custom hashing remain consistent.

## Separate Crypto Meaning

The `crypto` module's SHA/MD5/HMAC operations produce security-oriented
digests. They are unrelated to a runtime hash table's internal bucket hash and
do not make their input type eligible for maps or sets.

## Implementation

The type checker enforces the boundary whenever a map or set type is resolved,
before constructors, stdlib calls, reflection, or JSON policy. Repeated compiler
passes deduplicate the diagnostic by source span.

Coverage accepts `uint64`, `bool`, and refined-string collection identities and
rejects structs, enums, floats, and container keys. Previous tests that used
sets of records or raw JSON trees for unrelated JSON/reflection behavior now
use lists; primitive set JSON behavior remains covered.
