# Native follow-ups and language decisions for review

Native parity for this landing means the five fixture gates in
`tests/native_parity.json` passing on Windows MSVC at one committed revision.
The items below are follow-ups, not additional landing gates. Whole-language
parity and percentage estimates are retired. No work from this list starts here.

## Open native follow-ups from the coverage matrix

- Results/optionals: retain an alternate owned outer `default` for a borrowed payload until its lexical scope exits; fixture: `tests/native/borrowed_sum_outer_default.jett` (proposed).
- Results/refinements: compare a refinement constructor with a handled operand nested inside a binary expression; fixture: `tests/native/refined_handle_nested_operand.jett` (proposed).
- Projected collection views: preserve a borrowed field across an interface conversion and a later argument's handled failure; fixture: `tests/native/projected_view_interface_conversion.jett` (proposed).
- Reflection: compare wrong-owner and wrong-field-type diagnostics for a generic aggregate's reflected field read; fixture: `tests/native/reflected_aggregate_mismatch.jett` (proposed).
- Erased interfaces: compare exact-owner rejection and pending-error precedence for a primitive-backed refinement argument; fixture: `tests/native/interface_same_owner_pending_refinement.jett` (proposed).
- Resource sums: execute the existing absent optional self-reseat witness natively against its interpreter result; fixture: `crates/jett_mir/src/resource_ownership/fixtures/direct_optional_self_reseat.jett`.
- Resource sums: execute the existing failure-result self-reseat witness natively against its interpreter result; fixture: `crates/jett_mir/src/resource_ownership/fixtures/direct_result_self_reseat.jett`.

## Language rules selected or newly diagnosed since 2026-09-17

- `7b59b820`: verify/property checks establish their executed examples, not universally quantified correctness.
- `239f7242`: float32 rounds at each expression boundary; display widens the rounded value to the existing float64 carrier.
- `2dfe769e`: some/ok/fail consume owned payloads; keeping the original requires an explicit clone.
- `0bb6023c`: callable parameter view modes are part of its type; an explicit view cannot satisfy an owned parameter (E0375).
- `64b76d1f`: expected result context may specialize inferred never collection elements, but cannot invent absent generic parameters.
- `b89b2222`: function-valued calls evaluate arguments before the callee; anonymous callable types have no named-argument labels.
- `b89b2222`: creating a capability-accepting closure may be pure; invoking it still requires its declared capabilities.
- `c615002b`: callable parameters are contravariant, results covariant, and view modes exact.
- `4edc480a`: a generic method owner requires concrete type arguments; a bare generic owner is diagnosed.
- `769dbd37`: refinement predicates use their declaration context and fully coarsened input, not caller aliases or bindings.
- `6e787229`: interpolation selects Displayable for the expression's checked type and evaluates the receiver once.
- `2450e9b4`: one-line breakpoints show the current function's lexical bindings rather than flattened caller frames.
- `cf67fc2d`: typed secret debug values and reflection diagnostics redact secret payloads recursively.
- `7dffa420`: direct bytes/list/map/set/optional/result equality is rejected, including wrappers and generic/comptime uses (E0376).
- `12368e14`: enum payload structs require their exact explicit two-view Equatable.equals; no structural fallback.
- `68f5bbb1`: namespace constants are baked values with no startup initializer or global runtime storage.
- `dd010020`: mutable namespace declarations are rejected (E0377).
- `9554c637`: constant calculations require explicit comptime (E0378); unsupported constant value types fail conservatively (E9001).
- `3a70864c`: handling a refinement over a sum validates the whole sum; its validation error is a string.
- `aee75f32`: known move-only views cannot escape into owned returns/parameters, and returns-view annotations are rejected (E0401).
- `f0ec8030`: projected writes reject immutable/temporary roots (E0404) and view roots (E0401).
- `b7180bd9`: Resource-bearing getters/fields cannot manufacture owned copies or clones; owning constructors require owned payloads.
- `b1cf8943`: compiler debug events are separate from application stdout; release print/println are rejected (E0362).
- `1dde961d`: same-type actor equality compares instance identity; clones alias the instance and pending depth remains distinct.
- `7ff46cc7`: erased-interface concrete arguments must match the receiver's exact checked owner before conversion or method entry.
- `24f1b0bb`: reflected callable return refinement adapters validate lazily after one source call, without implicit joins.
- `9113f395`: an explicit comptime actor graph is an immutable template instantiated freshly per runtime evaluation, preserving internal sharing.
- `10e9c7e5`: interface-typed equality, including implicit enum traversal reaching interfaces, is rejected (E0376); explicit domain comparison remains available.
- `b612399e`: list.sum accepts numeric primitives and transparent aliases only; refinements, secrets and nonnumeric types are rejected.
- `132f9d98`: finite field replacement requires a live mutable owned record/exact-state machine and no dependent loan; bitfields and enclosing refinements require reconstruction.
- `132f9d98`: compiler reflection metadata is inspect-and-pass; projected writes cannot modify its builtin fields.
- `132f9d98`: matching a viewed enum preserves borrowed move-only payloads; coarsen/declassify do not create ownership.
- `a26523cc`: actor handlers preserve owned state at normal exits; Handle-ending respond is terminal and borrowed replies cannot escape.
- `f865e3f1`: a temporary-backed local view retains its whole producer until the original lexical scope exits, without last-use expiry or implicit cloning.
- `099571f8`: a persistent move-only alias blocks owner changes even after its last read; mutable owned backing grants no mutable view.
- `21245731`: Resource replacement stages the RHS before old cleanup; absent sums remain live owners and live shell/payload loans block replacement.
- `5d7bb8a0`: pure Verify/Property bodies may use Resource-typed values with no live Resource, without gaining capabilities.
- `7f7ca39b`: exact same-local direct Resource sum reseating retains occupancy and loans and adds no transfer, finalizer or clone.
