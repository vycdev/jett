# Jett Docs

This folder keeps the stable project references at the root and groups staging
notes by their current status.

## Core References

These files are intentionally kept in `docs/` and should be treated as the
canonical project overview:

- [Language design](design.md)
- [Compiler architecture](architecture.md)
- [Implementation progress](progress.md)

## Active Plans

These notes describe work that is still live or directly feeds the next JSON
and reflection implementation steps:

- [JsonValue to JsonTree transition](active/json_value_transition_plan.md)
- [JSON stdlib extraction plan](active/stdlib_json_extraction_plan.md)
- [Canonical reflection metadata plan](active/canonical_reflection_metadata_plan.md)
- [Stdlib visibility design](active/stdlib_visibility_design.md)

## Open Design

These notes capture choices that should stay deliberate until the language has
enough pressure from real code:

- [Initial C FFI and generated binding contract](open_design/c_ffi_binding_contract.md)
- [Type construction design](open_design/type_construction_design.md)
- [Type construction block syntax](open_design/type_construction_block_syntax.md)
- [Namespace-qualified type follow-up](open_design/namespace_qualified_types_followup.md)
- [State machine type model](open_design/state_machine_type_model.md)
- [JSON unknown field policy](open_design/json_unknown_field_policy.md)
- [JSON trusted hooks across backends](open_design/json_trusted_hooks_across_backends.md)
- [JsonValue primitive tag retirement](open_design/json_value_primitive_tag_retirement.md)
- [Prelude and root aliases](open_design/prelude_root_aliases.md)
- [Reflection predicate static folding](open_design/reflection_predicate_static_folding.md)
- [Print debug builtin policy](open_design/print_debug_builtin_policy.md)
- [uint64 runtime value model](open_design/uint64_runtime_value_model.md)
- [Initial `net.socket` transport contract](open_design/net_socket_transport_contract.md)
- [Random capability and entropy contract](open_design/random_capability_entropy_contract.md)
- [Encoding representation and failure contract](open_design/encoding_representation_failure_contract.md)
- [Crypto hashing and security contract](open_design/crypto_hashing_security_contract.md)
- [Time and Clock capability contract](open_design/time_clock_capability_contract.md)

## Completed Records

These documents are retained as historical implementation records. Some of them
describe blockers or plans that have since been resolved, so use the active
plans above for current work.

- [Breakpoint pause and inspection protocol](completed/breakpoint_pause_inspection_protocol.md)
- [Bitfield reflection metadata](completed/bitfield_reflection_metadata.md)
- [Bundle ordering contract](completed/bundle_ordering_contract.md)
- [Comptime type bind](completed/comptime_type_bind.md)
- [JSON public bridge handoff](completed/json_public_bridge_handoff.md)
- [JSON public parse policy](completed/json_public_parse_policy.md)
- [JSON raw access semantics](completed/json_raw_access_semantics.md)
- [JSON raw value design](completed/json_raw_value_design.md)
- [JSON reflection plan](completed/json_reflection_plan.md)
- [JsonTree decoder blocker](completed/json_tree_decoder_blocker.md)
- [Math numeric overload boundary](completed/math_numeric_overload_boundary.md)
- [Reflected construction staging](completed/reflected_construction_staging.md)
- [Reflection predicate facts](completed/reflection_predicate_facts.md)
- [Type kind design](completed/type_kind_design.md)

## Compatibility Stubs

- [Old JSON reflection plan path](json_reflection_plan.md), kept because
  `design.md` intentionally still points to the original location.
