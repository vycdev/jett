//! An aggregate is a new holder root, never a collection of copied raw tokens.
use super::super::checked::PreparedOwnedStruct;
use super::*;
use jett_parser::ast::Expr;

impl ResourceTransport {
    pub(crate) fn assemble_owned_struct(
        &mut self,
        proof: &PreparedOwnedStruct,
        expression: &Expr,
        actuals: Vec<EvaluatedValue>,
    ) -> Result<EvaluatedValue, ResourceExecutionError> {
        self.checked.revalidate_owned_struct(proof, expression)?;
        if actuals.len() != proof.source_fields().len() {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        // Validate every child before combining or migrating any holder. The
        // operation retains each reached child's cleanup path on any refusal.
        for (actual, ordinal) in actuals.iter().zip(proof.source_fields()) {
            if actual.custody.has_borrows() {
                return Err(ResourceExecutionError::InvalidOwner);
            }
            self.validate_value_type(actual, proof.fields()[*ordinal].1)?;
        }
        let mut fields: Vec<Option<Value>> = (0..actuals.len()).map(|_| None).collect();
        let mut custody = super::super::ValueCustody::default();
        // Preserve lexical acquisition order even when field destinations are
        // permuted. Struct storage order never dictates finalizer ordering.
        for (actual, ordinal) in actuals.into_iter().zip(proof.source_fields()) {
            let actual = actual.prefix(PayloadStep::Field(*ordinal));
            if fields[*ordinal].replace(actual.value).is_some() {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            }
            custody.append_owned(actual.custody)?;
        }
        let fields = proof
            .fields()
            .iter()
            .zip(fields)
            .map(|((name, _), value)| {
                value
                    .map(|value| (name.clone(), value))
                    .ok_or(ResourceExecutionError::InvalidPayloadPath)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut value = EvaluatedValue {
            value: Value::Struct {
                type_name: proof.name().to_string(),
                concrete_type: None,
                fields,
            },
            custody,
        };
        self.validate_value_type(&value, proof.ty())?;
        let holder = self.temporary(self.current_frame()?)?;
        self.ledger.transfer(&mut value.custody, holder)?;
        Ok(value)
    }
}

#[cfg(test)]
#[path = "owned_struct_tests.rs"]
mod tests;
