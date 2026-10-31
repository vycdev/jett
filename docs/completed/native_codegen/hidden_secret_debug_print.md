# Selected accepted debug print redaction

Status: implemented and verified in the integrated compiler. The native schema
and source admission policy are unchanged.

The interpreter uses typed recursive redaction for values already accepted by non-release
`print` and `println`. The alternative of blanket static rejection of interfaces
and construction builders would remove accepted public programs. Accepted debug
printing instead follows the existing trace/reflection token, `[redacted]`, for
secret-qualified subtrees. Renderers must inspect the qualifier before reading
the payload or any pending wrappers inside it. A public string equal to the
marker remains public text. No user getter, Displayable method or callback runs
while rendering.

The original checked argument type belongs to the caller. Capture it before any
argument evaluation, nested generic/scoped/reflected execution or ownership
transfer, then retain its association with the actual value. Interface values
use retained concrete nominal/generic identity; construction builders use each
stored field type. Missing composite/field metadata must produce a conservative
redacted subtree, never a raw payload fallback. Do not invent a concrete type
from matching fields or attach a destination type to erase the caller's secret
qualifier. Opaque callback and actor rendering preserves existing labels without
reading captures or actor state.

Arguments are evaluated once in source order before one complete print event.
`view` remains nonowning, ordinary arguments retain their existing consumption,
argument separators remain spaces, print adds no terminator and println adds
one newline. Diagnostic stderr/debug event transport remains unchanged.

Direct secret and refinement-wrapped secret Source restrictions, Resource-root
restrictions and Source release E0362 remain unchanged. Debug/Release runtime
archive profiles do not alter Source release policy. The native typed schema and
redaction guards remain independent safeguards and are not weakened to match
historical reference disclosure.

The Print/Println repair covers accepted debug observations. The accompanying
direct-field diagnostic successor applies existing secret non-disclosure to the
demonstrated pending secret receiver failure; both runtime profiles pass.
Pending-sum extraction and pipeline failure paths remain separate parity work in
docs/open_design/debug_print_hidden_secrets.md. No secret declassification or
additional source admission is introduced by either repair.

Original probe 07_hidden_secret_debug_print and control
15_public_erased_debug_print are preserved byte-for-byte. The historical
sentinel-secret disclosure is a defect observation, never the future oracle.
Expected selected debug text is app.Hidden(token: [redacted]) followed by one
newline; the public control retains app.Public(token: sentinel-public) followed
by one newline. Reference events and Source-deleted linked diagnostic output match these exact
bytes in both runtime profiles.
## Integrated verification

All 645 Comptime tests pass, including nine focused renderer controls. The
five native print controls pass: nine source programs produce 18 Source-deleted
executions across the normal Debug and Release runtime archives. The cases
cover hidden and public interface fields, generic owners, nested sums, builders,
pending collections, argument failure, owned/view routes and exact invocation
order. Source release and direct-secret rejection controls remain enforced.

All seven existing secret-owner controls pass in both profiles, including the
original pending secret receiver failure. Eight adjacent native debug and
pending-aggregate controls also pass. Exact stdout, stderr, failure status and
available cleanup checks remain asserted by the tests. No runtime ABI or native
renderer change is part of this reference repair.

The authentic reflected-callable test uses checked named and closure adapters,
verifies printing emits no callback or refinement-predicate events, then invokes
them explicitly. Its closure captures a copyable int64: Jett rejects move-only
secret captures. A separate host-value test covers defensive opaque rendering
of a synthetic secret capture without claiming source admission.

Logs: `target/native/secret-observation-comptime-full-v2.log`,
`secret-observation-native-both-v2.log`, `secret-observation-owner-views-v1.log`
and `secret-observation-adjacent-v1.log`. These focused results do not replace
full-workspace or supported-host release acceptance.

The existing raw-host public view test now supplies its known list[string]
contract. Its exact public text oracle is unchanged. Source argument facts are
captured from actual checked caller metadata before evaluation; no production
missing fact is synthesized to match that host-only setup. A separate control
requires conservative redaction when field/container metadata is unavailable.
Host-only renderer controls exercise defensive missing-metadata and secret-set
carriers; they do not claim those host values are accepted Jett Source. The
native renderer's malformed-schema/handle guards remain errors, not newly
redacted success. Exact Source output tests must prove checked facts exist for
public subtrees; conservative reference fallback alone is no parity credit.

The new pipeline/view fixture uses a named `emit(view value: Named)` wrapper
whose body calls `println(view value)`, followed by an owned `println(borrowed)`.
It preserves the two selected exact events. Both direct builtin pipeline
spellings, `borrowed into view println` and `borrowed into view println()`,
failed actual checking with existing E0313. Their original bytes and failure
logs are retained in the execution packet as before-policy observations. Direct
builtin pipeline admission is a separate feature-contract audit follow-up for
the full native goal; this change neither broadens it nor claims it covered.
