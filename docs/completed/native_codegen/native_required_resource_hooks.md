# Required Resource hook materialization

This change connects explicitly required, closed pure comptime evaluation to
native Resource hook values. It does not change function eligibility: every
semantically pure function remains eligible, including project and stdlib
functions, generic and scoped selections, arbitrary pure control flow, aliases
and relays. A runtime capability or provider is never available to that worker.

The shared reference oracle must evaluate required expressions using the same
CheckedResourceProgram identity before registering runtime code or installing a
provider. A failed evaluation or absent exact cache entry is an error; runtime
evaluation of the comptime body is not an acceptable oracle.

The existing public span/context cache is compatibility materialization data,
not Resource authority. An opaque proof is minted only from the private
program-bound checked required cache. It retains the original required-region
key, owner, outer expression occurrence and complete concrete specialization and
scoped selection. The same checked program and manifest must authenticate the
selected hook. A public mirror insertion, foreign program with identical Source,
copied or resealed expression, changed context, stale hook or missing evaluation
cannot mint that proof.

HIR consumes that opaque proof through an acyclic dependency on jett_comptime.
This precise compiler-internal coupling avoids a publicly forgeable tuple of
program, span and DefId. HIR preserves its immutable original ResourceSourceArchive
and records an authenticated materialized view of the original Comptime
occurrence. It must not overwrite the original Source, reseal a replacement as
original, or pretend required evaluation was a runtime call. MIR verifies that
mapping at the exact current occurrence and through canonical CFG/local remaps.

The current native value transport supports exact primitive literals, absent
optional Resource sums, and exact compiler hook descriptors. Qualified and
complex required values remain transport work; this does not restrict which
semantically pure functions may be evaluated. Required scalar rows also seal
ordinary runtime helpers without granting those helpers Resource frames or a
Resource calling convention.

Runtime Resource operations and ordinary descriptor values are preserved while
the baker walks the program. Only required values are materialized. Runtime
reachability must not force compile-time-only pure selector bodies through a
narrow returned-hook allowlist. Such a restriction would contradict existing
pure-function eligibility rather than implement native coverage.

The shared corpus grows from 46 to 61 scenarios using twelve new original
Source files. All 122 real-reference and 122 source-absent native runs pass in
debug and release against the unchanged matched MSVC archives. They cover all
three recipes, factory/domain/cleanup failure, alias, relay, unused storage,
immediate invocation, absent Resource sum, pure branch selection, generic/scoped
pure selection, and an ordinary runtime helper whose own required scalar value
keeps its normal calling convention. Reports require exact event order, outcome
channels, complete script consumption and zero custody before teardown.

Nine focused MIR controls and all 251 MIR library tests pass. Genuine pre-fix
controls exposed acceptance of a changed required scalar helper and omission of
disconnected Resource extraction headers; independent body sealing and typed
header detection now reject both. Exact sum-tag branches plan both syntactic
arms while the unchanged runtime branch selects absence or occupancy. Ordinary
non-Resource sums do not gain custody classification.

The complete codegen suite passes 100 library and 63 object-emission tests, and
all 93 driver-library tests pass. Rust/Jett formatting and diff checks pass;
all 2,168 frozen source/config inputs remain exact. The complete current
workspace, default native regression and supported-host jobs remain release gates.

Full native coverage remains active. Returned named/dynamic/captured callbacks,
broader Resource aggregates and views, production providers, concurrency, GNU
private execution, and coherent final workspace/distribution acceptance remain
separate outstanding requirements.
