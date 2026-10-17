# Source-only native Resource pipeline evidence

This records checked Source pipeline coverage for existing Resource behavior.

The expanded pipeline packet has measured local acceptance for **23 cases in
Debug and Release**. Its frozen-input receipt,
`target/native-resource-pipeline-source-evidence/acceptance-v2.json`, records
worktree HEAD `5d01097e5330d5aaa894c5fc92c786378f0e105a` and all **3,648 inputs
unchanged**; both measured archives and six nested receipts separately revalidate.
The input
snapshot identifies this expanded test packet; the recorded HEAD alone does not
establish acceptance of a later committed revision. Its measured
archive-receipt SHA-256 is
`29dd83c7f3874d7fe4b4fd3a5784786a5b31ee45337e9f40f133da9f33316d42`.

| Gate | Recorded evidence | Limit |
| --- | --- | --- |
| Checked Source reference execution | 46 pipeline attempts, included in the full 161-case / 322-profile-execution reference corpus; 522 reference tests pass. Exact handwritten outcomes and event order; zero outstanding owners and zero registry entries before teardown. | The reference observation tuple does not measure unconsumed script entries. Full reference acceptance does not prove full linked native acceptance. |
| Cranelift object preflight | 46 emitted objects through the checked Source/HIR/MIR path | Object emission does not establish linked execution. |
| Linked local native execution | 46 Debug/Release executions with Project and synthetic Stdlib Source files deleted before launch; exact status, messages and events; zero obligations before teardown and no remaining scripted inputs | Uses matched local Windows MSVC archives; does not establish GNU or other platform acceptance. |

The packet covers construction/move/close pipelines, written-view
retention, bare-owner view operations, named-argument abort ordering, domain
failure stopping the next step, and an indirect close descriptor. Its failure
rows check construction and borrow errors plus finalizer panic and retirement
before teardown. These are genuine checked Source programs; the scripted
provider selects outcomes for their runtime operations.

The two test-only additions now have measured Source reference and local native
acceptance:

| Accepted pipeline row | Measured existing behavior |
| --- | --- |
| `pipeline_bare_owner_provider_panic` | Provider panic retires the owner; native body/cleanup/channel `1/0/2`, exit `73`, message `runtime operation panicked`. |
| `pipeline_bare_owner_provider_panic_cleanup_wins` | Finalizer panic takes precedence over the provider panic; native body/cleanup/channel `1/255/3`, exit `71`, message `native Resource cleanup failed`. |

The earlier 21-case / 42-execution receipt remains historical evidence. The
preserved complete gate passes **161 Resource cases / 322 Source-deleted native
executions** in both profiles (1256.05 seconds). Its receipt is
`target/native-resource-pipeline-source-evidence/full-acceptance-v2.json`; all
3,648 frozen inputs, both measured archives and six nested receipts revalidate
unchanged. No GNU or other platform acceptance of the expanded test packet is
claimed here.

Broader Resource `For`/`Match`, reflected bodies and ordinary borrowed-data
families remain separate obligations. This supplemental packet leaves the fixed
`tests/native_parity.json` inventory at **207 fixtures / 182 run-pass lowering
obligations**, and leaves the broad planning estimate at **about 87%**. It does
not close the complete native-language or distribution gate.
