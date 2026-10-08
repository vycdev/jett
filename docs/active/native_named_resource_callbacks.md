# Named Resource callbacks

## Selected implementation contract

The next native parity milestone is an immutable function value initialized from
one original named, non-generic, capability-free function. Passing an owned
Resource to that value is existing Jett syntax. This work changes backend support;
it introduces no source-language restriction or new callable syntax.

The original checked invocation stays indirect. A separate private proof selects
the exact retained declaration from the original callable initializer and its
current use. Signature equality, a public function name or an edited MIR pointer
cannot select another body. Immutable alias chains preserve exact provenance;
ambiguous joins, reassignment and escapes require their own proved transport.

Reference execution must use the retained checked body and existing argument
custody envelopes, Return destination and operation floor. Native execution must
validate the selected code pointer and empty environment before entering the
existing Resource Scope bridge. Ordinary callable invocation omits that hidden
Scope and cannot be used for a Resource family target. Refusal must clean any
already acquired argument prefix without entering or publishing the wrong body.

## Original Source baseline, 2026-10-06

The shared `18_indirect_named_close.jett` parses, resolves and checks. Its named
`close_selected` owns `resource_probe.TestHandle`, calls the checked close wrapper,
and is bound to immutable `dispose` before construction of token 701.

The genuine reference gate reaches the first new scenario and fails with
`Resource source binding has no live cleanup custody`. The native gate reaches it
and fails MIR validation with `indirect custody call lacks an exact descriptor
producer`. These are executed failures, not accepted behavior. Baseline logs and
Source snapshots are retained in
`target/native-resource-indirect-named-source-integration/`.

## Required acceptance

The shared oracles add successful named close, handled construction failure,
ordinary target-body failure, close finalizer failure, and combined body/cleanup
failure. Original body status must survive cleanup failure, while cleanup
diagnostics take precedence.
A same-signature `wrong_target` performs an ordinary terminal failure; a separate
legal Source selects it and must execute that failure while retiring the Resource.
Private target-swap controls must refuse substituting that body into the checked
close program. This distinguishes exact body selection from matching signatures.

Both profiles must pass genuine reference and linked native execution, including
native execution after deleting the project and synthetic stdlib Source. Provider
events, body versus cleanup status, diagnostics, zero remaining obligations and
exactly-once teardown remain shared acceptance requirements. Existing 22 scenarios
and ordinary callable regressions stay required. Runtime ABI changes are not
planned; any runtime change requires newly built, measured matching archives.

## First reference gate

The new three private named-body tests pass, and the complete reference library
passes 511 tests. The shared gate executes all 28 scenarios in both profiles,
including exact target selection, immutable aliases and combined body/cleanup
failure: 56 reference executions with zero live provider obligations before
teardown. The corrected complete MIR suite passes 233 tests; native codegen passes 93
unit tests and 63 object-emission tests. The linked native gate passes all 28
scenarios in both profiles (56 executions), with Source absent and zero live
obligations before teardown. Existing hook descriptor storage remains unchanged;
the earlier broader companion predicate was rejected by its regression test.
Runtime sources and both measured archives still match receipt `bf0c7805`.
The complete driver library passes 92 tests. Ordinary callable regression groups
pass 13 function tests, one captured-closure test, one collection-callback test
and 16 caller-ownership tests. Fresh supported-host acceptance of this successor
remains a separate gate.

## Remaining full-goal work

Qualified namespace members are covered by the subsequent
[namespace callback proof](native_namespace_resource_callbacks.md). Returned
compiler hook descriptors, dynamic and captured callables, scoped and generic
bodies, reflected/pipeline/handled invocation and occupied aggregate
Resource paths remain required. Reference fixture07 returns a compiler hook
descriptor and does not prove named Source callback support. This milestone does
not count those separate invocation forms as covered.
