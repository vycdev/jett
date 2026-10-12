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

## Current integration verification

The restored integration includes the published actor identity change. Local
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
