# Initial HIR Lowering Plan

Status: accepted boundary; generic instantiations plus the first aggregate and
method slice are implemented for the current HIR subset.

Tracked by [#20](https://github.com/vycdev/jett/issues/20).

## Boundary

HIR consumes the semantic AST, `ResolveResult`, and `CheckResult`. The current
typechecker does not produce a second typed tree. Instead, `CheckResult`
provides checked expression and definition type maps plus the shared type
interner and reflection metadata. HIR materializes those facts into typed
nodes; it does not parse type syntax again or repeat source-language policy.

Resolver `DefId` and `TypeId` values are session-local join keys. They may be
stored in an in-memory HIR program, but canonical declaration identity is:

```text
DeclarationId = SourceOrigin + CanonicalNamespace + CanonicalName + DeclarationKind
FunctionIdentity = DeclarationId + ConcreteTypeArguments
```

The current in-memory `FunctionIdentity` uses concrete `TypeId` arguments and
is stable only inside one checked program. Persistent caches, linkage names,
and serialized HIR must replace raw `TypeId` values with canonical structural
type identities; they may not persist interner indices.

`SourceOrigin` is supplied explicitly for every source file. HIR never infers
authority from `FileId`, path spelling, namespace spelling, or a reserved
numeric range. Namespace aliases are lookup-only and never enter HIR identity.
An interface-implementation method identity includes both the concrete owner
and canonical interface so two same-named interface methods cannot collide.

Every HIR expression has one checked `TypeId` and source span. Every function,
parameter, and local has a deterministic dense HIR ID assigned in canonical
source/discovery order. Source spans preserve the current provenance chain;
the later CST layer can extend that chain without changing HIR semantics.

## Phase Ownership

The typechecker owns language validity, type compatibility, capability policy,
copyability, early ownership diagnostics, reflection facts, and trusted-stdlib
authorization. HIR consumes accepted facts and makes execution choices
explicit. It does not turn runtime booleans into type proofs or recognize
trusted hooks by spelling.

HIR owns deterministic concrete-function discovery, monomorphization,
materialization of checked method targets, removal of syntax-only forms,
explicit execution operations, and a backend-neutral typed input for MIR. The
typechecker selects the legal concrete source method because interface dispatch
depends on checked argument types; HIR assigns that body a deterministic
`FunctionId`. MIR owns control-flow graphs, definitive ownership dataflow, drop
placement, and backend-independent validation. Backend adapters may not
redefine HIR language policy.

## Generic Instantiation Contract

The next typechecker handoff is an ordered checked-instantiation manifest. Each
entry identifies the generic declaration by session-local `DefId`, lists
canonical concrete `TypeId` arguments, resolved parameter and return types,
and preserves per-instantiation expression-type facts for body lowering.

The canonical HIR identity is declaration plus concrete type arguments only.
Call-site reflection facts can authorize conservative checking but do not
create alternate function identities. Facts derived directly from a concrete
type may specialize HIR; arbitrary caller values remain runtime values.

Discovery begins at non-generic roots and accepted explicit or inferred
generic calls. It is deterministic, deduplicates repeated instantiations, and
reaches a fixed point for nested calls and recursion. Rejected calls never
enter the manifest. Recursive discovery reserves identity before lowering.

## Implementation Stages

1. **Implemented:** `jett_hir` data model, canonical declaration/function
   identity, explicit source-origin handoff, checked definition types, and
   typed lowering for ordinary top-level functions, parameters, locals,
   literals, unary/binary expressions, direct user calls, assignment, return,
   `if`/`else` normalization, `while`, `break`, `continue`, views, and clones.
2. **Implemented:** export the ordered, deduplicated generic-instantiation
   manifest and per-instantiation expression types and nested-call targets from
   `jett_typecheck`; lower explicit, inferred, repeated, nested, and recursive
   generic calls to concrete HIR function identities.
3. **In progress:** named arguments, source-defined method targets, struct
   fields and constructors, list/map construction, and unhandled pipelines now
   lower into canonical core forms while retaining lexical left-to-right
   evaluation. Pipelines become nested calls to checked ordinary, generic, or
   concrete source-method targets; their synthetic piped argument participates
   in the checked parameter permutation. The checker exports those
   permutations, concrete method bodies, constructor targets, and
   refinement-validation requirements, including separate facts inside generic
   instantiations. Result/optional construction and result, optional, and
   refinement-boundary handles now lower explicitly. `default` yields from the
   failure block while `return` exits the function; handled pipeline steps use
   this same form around their intermediate call. Bitfield/machine fields and
   constructors, compiler intrinsics, matches, remaining collection operations,
   reflection, and trusted calls remain staged.
4. Add deterministic HIR snapshots and wire HIR into the driver after every
   accepted source construct either lowers or has an explicit staged error.
5. Freeze the HIR validator and begin the HIR-to-MIR contract in #22.

The current tree-walking interpreter remains the execution path during these
stages. HIR is not a second source-language implementation and does not yet
change observable Jett behavior.
