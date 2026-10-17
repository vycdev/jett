//! Candidate identity tests; not executed in this preparation phase.

use super::*;

fn record(name: &str) -> StructDef {
    StructDef {
        name: name.to_owned(),
        fields: vec![("index".to_owned(), TypeInterner::INT64)],
        methods: Vec::new(),
    }
}

#[test]
fn metadata_classification_uses_allocation_identity_not_spelling_or_shape() {
    let mut types = TypeInterner::new();
    let builtin = types.add_compiler_metadata_struct(record("TypeField"));
    let lookalike = types.add_struct(record("TypeField"));
    assert_ne!(builtin, lookalike);
    assert!(types.is_compiler_metadata_struct(builtin));
    assert!(!types.is_compiler_metadata_struct(lookalike));
    assert_eq!(
        types.resolve_struct(builtin),
        types.resolve_struct(lookalike)
    );
}

#[test]
fn snapshots_preserve_exact_metadata_markers_and_independent_table_growth() {
    let mut original = TypeInterner::new();
    let builtin = original.add_compiler_metadata_struct(record("TypeField"));
    let lookalike = original.add_struct(record("TypeField"));
    let mut snapshot = original.snapshot();
    assert!(snapshot.is_compiler_metadata_struct(builtin));
    assert!(!snapshot.is_compiler_metadata_struct(lookalike));
    snapshot.update_struct(builtin, record("RenamedByCompilerTransform"));
    assert!(snapshot.is_compiler_metadata_struct(builtin));
    assert_eq!(original.resolve_struct(builtin).name, "TypeField");
    let new_user = snapshot.add_struct(record("TypeField"));
    assert!(!snapshot.is_compiler_metadata_struct(new_user));
    assert!(!original.is_compiler_metadata_struct(new_user));
}
