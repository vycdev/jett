# Direct equality of collections and sums

Settled on 2026-09-30: reject direct `==` and `!=` on bytes, lists, maps, sets,
optionals, and results at compile time. The former checker accepted these
expressions, while the interpreter failed at runtime and native codegen rejected
them internally. These unsupported comparisons now report E0376 before comptime
evaluation or backend lowering.

Classification follows transparent aliases, refinement ancestry, and secret
wrappers. The same rule applies to concrete generic instantiations and explicit
comptime expressions, including absent sums and empty collections. It does not
introduce structural equality, change collection-key eligibility, or change the
separate existing collection comparison path within enum payloads. Struct,
actor, and erased-interface comparison contracts remain separate.

Four compile-fail fixtures cover both operators, all six families, aliases,
inherited refinements, secrets, concrete generics, and explicit comptime values.
The native publication regression checks every family and operator in debug and
release, requires the frontend diagnostic, and preserves existing artifacts.
