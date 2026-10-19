# Projected field assignment: selected local owner updates

Status: decided on 2026-10-15 after the user delegated language choices to fit Jett's philosophy. Checked updates through owned mutable values are selected. Implementation and verified acceptance are tracked in the [native acceptance audit](../completed/native_codegen/native_acceptance_audit.md); this selection alone is not evidence that either backend executes projected writes.

Jett's imperative/local-mutability and small logical diff goals favor `record.field = value` for an exact live mutable owner, rather than rebuilding every unrelated constructor field for a one-field repair. Immutable defaults, explicit consuming transforms and read-only views remain. The full selected contract and implementation obligations live in [owned mutable field updates](../completed/native_codegen/owned_mutable_field_updates.md).

The eligible place is a finite checked field path with exact resolved binding, nominal/generic owners and field types. Existing state qualification or checked narrowing may supply an exact machine field; no state transition is implied. No view, mutable reference, temporary, consumed owner or outstanding dependent loan authorizes a write. No qualifier peeling or structural field match supplies ownership proof.

An enclosing nominal refinement is replaced and validated as a whole instead of edited internally. A complete refined field may be replaced under its existing checked conversion rules. Bitfields retain their constructor-and-rebind contract, including declared width validation; the former table entry proposing direct masked assignment is superseded.

The existing compiler-produced reflection metadata authority is preserved.
Projected writes do not enter canonical builtin metadata owners to edit their
fields. Capture exact builtin identities at creation and retain them through
type-table cloning/remapping; aliases, qualification and user holders cannot
remove the boundary. Whole genuine metadata fields remain replaceable in user
holders, and user types with similar names remain ordinary records. This
selection precedes implementation and introduces no new observer-provenance API.

Preflight the place and ancestor readiness/state before the RHS. Existing pending inspection errors remain runtime behavior for admitted pending transport; no implicit join or payload read is added. Evaluate the RHS once, preserve the old field on precommit failure, and transfer/retire replacement ownership exactly once. Preserve untouched fields, metadata, task depth and logical clone independence. Resource-bearing paths need their authenticated custody protocol and cannot use an ordinary update as a bypass.

Historical baseline: the owned mutable integer/String probe passed checking, while reference execution reported `only simple variable assignment is supported in comptime` and MIR rejected a nonlocal assignment without materialized ownership. Existing immutable/view/temporary rejection and projected-read/reconstruction controls remain authoritative. Their retained Source bytes and historical receipts must not be rewritten to turn this decision into a pass.

Ordinary-record and exact-state machine updates now have checked reference, required-comptime and bounded Source-absent native acceptance. Actor handler state, Resource-bearing replacement and wider reflected/generic compositions remain implementation work. Full coherent workspace, inventory and supported-host acceptance remain separate gates; the audit records their actual revision and scope.
