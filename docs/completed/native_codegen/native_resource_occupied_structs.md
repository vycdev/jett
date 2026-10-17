# Occupied Resource struct constructors

Checked named construction preserves written field evaluation order and the
original nominal schema. Begin/Stage/Commit/End carry a private constructor
witness through MIR and codegen. Original/current body and operation checks
reject forged public nodes before pruning. Slot and carrier migration keep one
cleanup obligation per owned Resource. Normal and abandoned-prefix cleanup
retire owners before finalizers, in reverse acquisition order.

Sources33..35 cover ordinary scope drop, reversed named argument order, and
abandoned construction after a Resource has staged. Sources30..32 preserve the
historical invalid view-capability entry and its E0503 refusal. The shared support
Sources and original Source17 remain unchanged.

## Resource predecessor verification

At Resource commit `c141de22`, the integration includes the published actor
identity change. Local
MSVC static-CRT checks on 2026-10-12 pass all 308 runtime, 577 interpreter, 317
MIR, and 120 code generator unit tests. The three-Source object gate emits all
six objects with both codegen optimization settings. Focused tests validate
constructor provenance, lexical staging, copied-node and forged-header refusal,
exact owner migration, stale generations, atomic late refusal, and cleanup that
continues after finalizer panic.

Fresh linked acceptance passes on 2026-10-12 against independently measured
private Debug/Release MSVC static-CRT archives with both cfgs and ABI1/wire3.
The occupied retirement gate passes 22 executions/22 entries; same-grant reentry
passes 22/44. The original twelve predecessor gates pass 604/728 against the
same archives. Additional pipeline/control-flow subset gates pass 106/106,
so all sixteen Resource gates pass 754 native executions/900 entry completions. Each
gate removes its project and synthetic stdlib Sources before execution and checks
exact script events, failure precedence, statuses, messages, process exits, eight
zero custody counts and empty ordinary storage before one teardown.

The MSVC measurement helper passes all 60 Python controls. It measures actual
compiler/linker commands, ordered native libraries, complete COFF members and
exact runtime exports. Cargo reports a freshly compiled gate; its dep-info
contains the independently supplied receipt pin exactly once. Runtime receives
only the receipt path. Source, archive, compiled gate and dep-info guards pass
after execution. The local predecessor collector was repaired to recognize its
two actual summary formats, then validated the completed logs without rerunning
the native tests. Accounting distinguishes the original twelve-gate corpus from
the subset and occupied gates. All 2,314 workspace library tests pass. This
evidence does not include an independent second raw-report
decoder or retain thousands of per-case executable copies.

## Combined revision and executable reuse

Fresh verification on 2026-10-12 includes Actor, Resource and Owner commit
`7ff46cc7` plus the executable-reuse harness change. All sixteen private MSVC
gates pass again: 754 Source-deleted executions and 900 entry completions.
The newly measured Debug/Release ABI1/wire3 static-CRT runtimes include the
additive Owner validation leaf. Receipt SHA-256 is
`bc2ad54743380441f074d8d5d212048ecf440669817c0cb69ce2fd0810ba1c95`.
Fresh gate compilation, source/archive/tool/binary/dep-info guards and the
runtime-environment pin refusal control pass. The combined implementation
passes all 2,331 workspace library tests; this is not full native conformance.

Consecutive script cases with identical project bytes reuse one completed
program within the exact runtime profile. Every case still runs in a fresh
process and working directory, after Source removal and binary-hash validation,
with its own script, report and unchanged oracle. Compiler-session proofs and
runtime state are never cached. The 191-case gate retains 382 executions while
reducing emitted/linked programs from 382 to 246. Occupied retirement retains
22 executions while reducing builds from 22 to 6. Reentry gates are unchanged.

An additional evidence-enabled occupied run passes all 22 cases. Independent
file checks verify each executable, object, original script, report, exit and
empty capture by size and SHA-256. Compact records retain report/script bytes
and six distinct executable hashes; temporary per-case executable copies were
pruned after verification. These checks validate report framing, not a second
full event decoder. Source absence is asserted by the passing guarded harness.
All wider language, provider and platform obligations below remain open.

## Historical checkpoint

The isolated v16 checkpoint recorded 22 executables/22 entries for occupied
scope retirement and 22/44 for same-grant reentry. Together with its twelve
predecessor gates, it recorded 648 executables/794 entries on private MSVC
archives. Those historical target receipts were discarded during disk cleanup;
they do not establish acceptance of the restored integration.

## Remaining scope

The current eleven script cases do not observe completed fields, stage owning
strings or nested occupied carriers, or exercise occupied constructor allocation
refusal. Pair factory failures happen before construction; abandoned-prefix
coverage uses a later ordinary marker handler return. These are remaining
native completion obligations, along with projection, extraction, replacement,
reflection, recursive/generic aggregates, collections, captured lifetimes,
broader Default compositions, and GNU/installed-distribution acceptance.

Private scripted execution does not establish public Driver admission or a
normal-runtime Resource provider. Backend refusals remain incomplete coverage.
The fixed inventory is 207 fixtures/182 object obligations; overall native
coverage is not complete.
