//! Exact statement endpoints for the one compound whole-binding operation.
use super::*;

fn index(
    operations: &[&jett_mir::ResourceOperation],
    selected: &jett_mir::ResourceOperation,
) -> Result<usize, CodegenError> {
    operations
        .iter()
        .position(|operation| std::ptr::eq(*operation, selected))
        .ok_or_else(|| pending("Resource replacement endpoint is outside its exact current site"))
}

pub(super) fn validate_rows(
    function: &jett_mir::ResourceFunctionPlan,
    operations: &[&jett_mir::ResourceOperation],
    replace: &jett_mir::ResourceOperation,
    transfer: &jett_mir::ResourceOperation,
) -> Result<(), CodegenError> {
    let expected = function
        .operations()
        .iter()
        .filter(|operation| {
            operation.site() == replace.site() && !operation.is_expression_operation()
        })
        .collect::<Vec<_>>();
    if operations.len() != expected.len()
        || operations
            .iter()
            .zip(&expected)
            .any(|(actual, expected)| !std::ptr::eq(*actual, *expected))
    {
        return Err(pending(
            "Resource replacement changed its complete current endpoint order",
        ));
    }
    let Role::Replace {
        destination,
        replacement,
        old,
    } = replace.role()
    else {
        return Err(pending(
            "Resource replacement lacks its exact Replace constructor",
        ));
    };
    if !matches!(transfer.role(), Role::Transfer { source, destination: selected }
        if source == replacement && selected == destination)
    {
        return Err(pending(
            "Resource replacement lost its exact evaluated RHS transfer",
        ));
    }
    let mut drops = operations.iter().copied().filter(
        |operation| matches!(operation.role(), Role::Drop { source, .. } if source == destination),
    );
    let drop = drops
        .next()
        .ok_or_else(|| pending("Resource replacement lost its displaced-value cleanup"))?;
    if drops.next().is_some()
        || !matches!(drop.role(), Role::Drop { occupancy, .. } if occupancy == old)
        || index(operations, replace)? >= index(operations, drop)?
        || index(operations, drop)? >= index(operations, transfer)?
    {
        return Err(pending(
            "Resource replacement changed its exact cleanup/transfer correspondence",
        ));
    }
    Ok(())
}
