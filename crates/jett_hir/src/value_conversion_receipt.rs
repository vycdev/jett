//! Ephemeral proof of one private canonical coercer execution, not a reseal API.
use crate::{LowerError, Program};
use jett_common::{FileId, Span};
use jett_types::TypeInterner;

struct Conversion<'types> {
    types: &'types TypeInterner,
    before: Program,
    after: Program,
}

/// No HIR node or archive stores this temporary producer receipt.
/// The private boxed snapshots cannot refer back to the newly created receipt.
pub(crate) struct CanonicalValueConversion<'types>(Box<Conversion<'types>>);

fn metadata_equal(left: &Program, right: &Program) -> bool {
    left.resource_source == right.resource_source
        && left.resource_manifest == right.resource_manifest
        && left.equality_methods == right.equality_methods
}

fn error(program: &Program, message: &str) -> Vec<LowerError> {
    vec![LowerError {
        span: program
            .functions
            .first()
            .map_or(Span::new(FileId::new(0), 0, 0), |function| function.span),
        message: message.into(),
    }]
}

/// Visible only to interface_values, whose completion path is the sole
/// production caller. No caller supplies an output table or conversion callback.
pub(super) fn coerce<'types>(
    program: &mut Program,
    before: &Program,
    types: &'types TypeInterner,
) -> Result<CanonicalValueConversion<'types>, Vec<LowerError>> {
    if !metadata_equal(program, before)
        || !crate::resource_materialization::functions_equal(&program.functions, &before.functions)
    {
        return Err(error(
            before,
            "canonical conversion changed its complete input before execution",
        ));
    }
    super::coerce_program(program, types);
    if !metadata_equal(program, before) {
        return Err(error(
            before,
            "canonical coercer changed protected non-function metadata",
        ));
    }
    Ok(CanonicalValueConversion(Box::new(Conversion {
        types,
        before: before.clone(),
        after: program.clone(),
    })))
}

impl CanonicalValueConversion<'_> {
    /// Authenticate the complete producer preimage and retain every actual
    /// occurrence-specific callable token, including appended adapter bodies.
    pub(crate) fn replay(
        &self,
        before: &Program,
        types: &TypeInterner,
    ) -> Result<Program, Vec<LowerError>> {
        if !std::ptr::eq(types, self.0.types)
            || !metadata_equal(before, &self.0.before)
            || !crate::resource_materialization::functions_equal(
                &before.functions,
                &self.0.before.functions,
            )
        {
            return Err(error(
                before,
                "canonical conversion receipt has another complete input or type arena",
            ));
        }
        Ok(self.0.after.clone())
    }
}

#[cfg(test)]
#[path = "value_conversion_receipt_tests.rs"]
mod tests;
