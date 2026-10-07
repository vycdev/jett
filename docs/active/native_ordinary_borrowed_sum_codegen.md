# Native ordinary borrowed optional/result payloads

The native-codegen goal remains active. This change implements the existing
ordinary handled-view family; it does not establish whole-language parity.
The [Source-only baseline](native_ordinary_borrowed_sums.md) retains its original
12 reference passes and 24 native unsupported-handler refusals as history.

## Contract and implementation

The canonical initializer is an explicit outer `view` of a Handle targeting an
explicit `view` of one stable immutable sum local or View parameter. The absent
continuation ends with Return. A Some/Ok move-only payload keeps its exact checked
type and backing dependency without a clone, sum transfer, or owning cleanup
slot. Forwarded, nested and loop aliases preserve that dependency. Explicit
`clone` still acquires independent ownership.

Implicitly copyable final destinations keep ordinary copy semantics. Primitive
values are copied; String owns its retained copy. A copied destination may be
mutable and rebound. After initialization, generated projection intermediates
leave the borrow inventory, allowing a later ordinary transfer or owning unwrap
of the original sum. This does not expire a persistent move-only alias or permit
changing its backing owner. A Result Fail String companion likewise owns one
retained copy for its handler.

MIR seals the original declaration, current headers, backing identity, tag
observation, selecting branch, success/failure projections and exact final
initializer. Canonical remapping authenticates both the prior and resulting
graphs; exact no-op transitions preserve proof identity. Missing, copied,
foreign, resealed or disconnected witnesses cannot authorize payload views.
Native admission validates this proof before consuming sum lowering.

The nonconsuming `jett_rt_v1_sum_payload_borrow` leaf returns the selected payload
bits without allocation, clone, retain, adoption or mutation. Generated source
and success payload locals remain nonowning. Only the checked copy destination
acquires ordinary ownership. Scalar pending depth and opaque container metadata
are preserved. Outer pending sums fail before extraction or continuation;
ready sums retain later payload-observer readiness checks.

This bounded proof grants no Resource custody. Alternate outer Default payload
lifetimes, mutable backing origins, temporary-backed aliases, general loan expiry
and payload identity hidden by erased interfaces, callables, actors or capability
carriers remain separate work. Nested scalar Handle defaults retain their
existing semantics.

## Measured acceptance

The coherent candidate based on `c453297d` passes:

| Gate | Result |
| --- | --- |
| Original frozen list-payload packet | 12 checked reference Sources; 24 Source-deleted debug/release native executions |
| Separate supplemental packet | 3 checked reference Sources; 6 Source-deleted debug/release native executions |
| MIR and native codegen | 276 MIR, 107 codegen, 63 object and 1 CFG checks |
| Runtime | 223 checks, including 8 nonconsuming payload controls |
| Driver library | 93 checks |
| Resource regression with fresh matched runtime archives | 138 scenarios; 276 Source-deleted debug/release native executions |

The final combined Source gate passes all four Rust groups in 79.12 seconds.
It compares complete status/stdout/stderr and requires empty debug streams.
The Sources cover Some/None and Ok/Fail, stable owned/View-parameter roots,
forwarding, nested aliases, real loops, clones, named argument-prefix Return and
terminal failure, outer pending sums and pending payloads. Supplemental cases
add a later argument Handle Default failure after a staged alias, plus mutable
copied integer/String bindings followed by original owning unwrap. They preserve
the original Sources, runner and baseline note; their parent registration is
an additive change, separately recorded from the frozen baseline artifact.

The final compiler/runtime/Source snapshot contains 2257 inputs, independently
revalidated after execution. Its manifest SHA256 is
`20c4c6b5ea49b94f16ed8f68b6db4aa913909227cbfce8571e403d78077e8a94`.
Evidence is retained under `target/native-ordinary-borrowed-sum-candidate-evidence/`
in `acceptance-v7.json`, `driver-all-packets-v7.log`, `mir-codegen-full-v5.log`
and `fresh-runtime-resource-acceptance.json`.

Fresh Resource archive receipt SHA256 is
`29dd83c7f3874d7fe4b4fd3a5784786a5b31ee45337e9f40f133da9f33316d42`.
The receipt is pinned at test compilation; runtime environment chooses only its
path. The 138-scenario gate passes in 761.14 seconds with Source absent at launch,
exact reference events/status/messages and zero obligations before teardown.
Both archives, all nested receipts, the frozen test executable and current
compiler/runtime/Source inputs are rehashed after the run. Subsequent changes to
compiled inputs are limited to supplemental tests, their formatting, and a
codegen test symbol lookup; documentation changes do not affect that gate.

Reproduce the public gates from the repository:

```text
cargo test --locked -p jett_mir -p jett_codegen_cranelift
cargo test --locked -p jett_runtime
cargo test --locked -p jett_driver --lib
cargo test --locked -p jett_driver --test native_conformance ordinary_borrowed_sums -- --nocapture --test-threads=1
```

The private Resource gate additionally needs the independently measured archive
receipt and its compile-time pin. See [archive measurement](native_resource_gnu_gate.md)
and the [Resource runner](../../crates/jett_driver/tests/native_conformance/resource_execution.rs).

## Remaining release gates

The estimate advances from about 86% to about 87% as this concrete family gains
linked native coverage. It is a planning estimate, not measured code coverage;
the fixed 207-fixture/182-object inventory denominators do not change.
A coherent current full workspace and all four supported-host workflow jobs,
including GNU private Source execution, remain required. Earlier green revisions
do not prove this runtime/compiler revision. The complete 100% objective and
all other semantic, ordinary-carrier, Resource and reflection obligations remain
open in the [acceptance audit](native_acceptance_audit.md).
