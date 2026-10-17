# Native same-local Resource sum reseating

This selected compiler-only extension admits `token = token` for the exact same
mutable owned Resource local. Existing occupied plain Resource reseating stays
valid. Direct `optional[R]` and `result[R, E]` shells are selected for every
checked Empty/Occupied/Conditional occupancy, including live `none` and `fail`
shells. A result failure retains its exact ordinary error companion.

Admission must preserve the original Source identity, complete nominal Resource
and failure types, local slot, containing lexical scope, current activation and
existing loan checks. Reseating itself creates no runtime replacement, transfer,
finalizer or implicit clone. Scope cleanup still retires the live owner and any
ordinary companion once. Generic, aggregate and projected carriers are outside
this selected whitelist. No Runtime, public ABI or report-protocol change is
selected. Source eligibility and capability restrictions remain unchanged.

## Original witnesses and recorded baseline

| Original fixture | Bytes | SHA-256 |
| --- | ---: | --- |
| direct_optional_self_reseat.jett (`none`) | 272 | 73482d983e29bffc61e2f6829c26f8f4514e10f2560bd0481cebe0f887271c5a |
| direct_result_self_reseat.jett (`fail("kept")`) | 282 | 4d4357096c172d7a4ac4cebe53dd2451b0f97ee600d6fb5525714f07e22756ae |

Both fixtures remain unchanged under crates/jett_mir/src/resource_ownership/fixtures/.
One existing plain-Resource control passes. One reference control passes both
original Sources across its two profiles. Those are one Rust control each, not
new native execution counts. The split MIR baseline fails two tests and the
object-emission baseline fails two tests at the exact plain-only reseat refusal:
`self-rebind needs the exact occupied plain Local reseat`.

The initial combined MIR failure reaches only the optional witness in Debug.
The later split tests establish both original witnesses at their first Debug
refusal; none of these red records grants a Release-attempt or emitted-object
pass. Log paths below are relative to target/native/:

| Baseline log | SHA-256 |
| --- | --- |
| resource-sum-self-reseat-plain-baseline-v1.log | dacea5d0cc069102479595389dabe5f40536ed3eeb01f5866e8b3a1673f3ebf9 |
| resource-sum-self-reseat-reference-baseline-v1.log | a794c672934dccea06b97b964628cae1b7a2eeff61e6fd3ec5bea612215d31da |
| resource-sum-self-reseat-mir-red-v1.log | 91631222a54076acc220c7bbec2bf0aea3b9eb72d1cf2ee5070821ba4888d256 |
| resource-sum-self-reseat-mir-red-v2.log | ffc72cb8a00cdcaf7e7d1caf2e7fd6d311b424de0e2191b959692cb3d2e789c0 |
| resource-sum-self-reseat-object-red-v1.log | efddf873675880989451780e4a90f0a6671949bb00dfc6f5dc5398c7038c9010 |

## Core compiler checkpoint

The actual MIR flow now admits the selected optional/result occupancies through
its existing exact same-Local branch. Both unchanged original MIR controls and
both object-emission controls pass, with each helper completing Debug and
Release. The MIR controls retain one exact SelfRebind and no Replace, Transfer
or Drop at that assignment site. This is object-emission evidence with **zero
linked native execution credit**. It does not by itself establish occupied or
conditional execution, observable failure-companion preservation or loan refusal.

Broad local regression passes **1,291 distinct checks**: 142 Codegen library,
66 Codegen integration, 694 Comptime library, 388 MIR library and one MIR
integration. The focused green checks are included, not added again. Current
logs/receipt pins are:

- MIR green: f18b1257baed6f55bfebcd745e26adc56ec4e11d9915e91faa2f9de28d6dabe6.
- Object green: 3339d1dd9c25ab27ae5482ca68431a92c4edf0a96508e4c4b1dfab9156b0f21d.
- resource-sum-self-reseat-core-regression-v1.log:
  26ae9be8603af75a518c9aaad9eb212748a017e71ea58350feec633729400add.
- resource-sum-self-reseat-mir-repair-v1/local-verification.json:
  81fb340662dccb6c8d2d78d610e7d8cd4c7b69832f878656c3cbf9b695d45115.

## Acceptance still required

Linked native execution, occupied/conditional positive cases, unknown/forged
same-owner refusals, observable companion preservation and live-loan refusal
evidence remain **pending**. Their separate follow-up controls are outside this
core checkpoint. Generic, aggregate and projected carrier admission remains
conservative. No current fresh compiler gate or full ordinary corpus is accepted
for these changed compiler inputs. Earlier replacement/suite receipts retain
their own scope. Existing native inventory and workspace validation remain pending for these
compiler inputs. Additional lifetime instrumentation and further platform
certification are separate follow-up work and do not expand the frozen
implementation coverage goal. Historical broader acceptance flags remain false;
CI stays disabled.

See [the design](../../design.md) and [architecture](../../architecture.md).
