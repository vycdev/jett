//! Factory-authenticated Verify/Property transitions use ordinary copy words.
//! The shared runtime activation row grants no Source call certificate.
use super::*;

impl Rows<'_, '_> {
    pub(super) fn native_suite_row(
        &mut self,
        caller: &custody::ResourceFunctionPlan,
        frame: u32,
        invocation: &custody::ResourceNativeSuiteInvocation,
    ) -> Result<Vec<u32>, CodegenError> {
        let target = invocation.function();
        let proof = invocation.proof();
        let formals = invocation.formals();
        let order = invocation.evaluation_order();
        let operands = invocation.operands();
        let result = invocation.result();
        let callee = self
            .plan
            .function(target)
            .ok_or_else(|| pending("native suite target is outside its exact fresh family"))?;
        if proof.owner() != caller.function()
            || proof.target() != target
            || proof.evaluation_order() != order
            || proof.arguments().len() != formals.len()
            || formals.len() != callee.parameters().len()
            || order.len() != formals.len()
            || operands.len() != formals.len()
            || callee.provisional_return().is_some()
            || callee.return_type() != TypeInterner::NOTHING
            || !matches!(result, CallResult::Ordinary { ty } if ty == TypeInterner::NOTHING)
        {
            return Err(pending(
                "native suite invocation changes its factory header or ordinary result",
            ));
        }
        let scope = self.frame(target, callee.root_scope().id())?;
        self.frames[scope as usize]
            .parents
            .push(Parent::Frame(frame));
        // Wire row 16 is only a runtime activation protocol. Its compiler
        // authority remains the distinct InvokeNativeSuite receipt above.
        let signature = self.function_signature(target)?;
        let mut row = vec![16, frame, target.index(), signature, scope, 0];
        row.push(count(order.len())?);
        for parameter in order {
            row.push(count(*parameter)?);
        }
        row.push(count(formals.len())?);
        for (parameter, param) in callee.parameters().iter().enumerate() {
            let fact = formals
                .get(parameter)
                .filter(|fact| fact.parameter() == parameter)
                .ok_or_else(|| pending("native suite formal is absent or reordered"))?;
            let original = proof
                .arguments()
                .get(parameter)
                .ok_or_else(|| pending("native suite has no exact generated argument"))?;
            if param.mode != jett_mir::ParamMode::Owned
                || fact.access() != jett_mir::ParamMode::Owned
                || fact.effect() != custody::ResourceArgumentEffect::Copy
                || fact.syntax() != custody::ResourceArgumentSyntax::Bare
                || fact.actual_type() != param.ty
                || fact.parameter_type() != param.ty
                || original.ty != param.ty
                || order.get(fact.source_index()) != Some(&parameter)
                || !matches!(operands.get(parameter),
                    Some(Operand::Ordinary { parameter: actual, ty })
                    if *actual == parameter && *ty == param.ty)
                || !matches!(
                    self.plan.types().resolve(param.ty),
                    Type::Int8
                        | Type::Int16
                        | Type::Int32
                        | Type::Int64
                        | Type::Uint8
                        | Type::Uint16
                        | Type::Uint32
                        | Type::Uint64
                        | Type::Float32
                        | Type::Float64
                        | Type::Bool
                        | Type::String
                        | Type::Nothing
                )
            {
                return Err(pending(
                    "native suite formal is not its exact ordinary copy parameter",
                ));
            }
            // No Graph Source locator or Resource carrier adaptation participates
            // in this generated bridge. Both ordinary types must be exact.
            let shape = self.shape(param.ty)?;
            row.extend([
                count(parameter)?,
                count(fact.source_index())?,
                shape,
                shape,
                1, // bare syntax
                1, // copy effect
                1, // owned formal access
                1, // ordinary value
            ]);
        }
        row.extend([1, self.signatures[signature as usize].result]);
        Ok(row)
    }
}
