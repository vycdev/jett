use super::*;
use crate::value::ClosureTypeContext;
use jett_common::{FileId, SourceOrigin};
use jett_parser::ast::Stmt;
use jett_typecheck::CheckOptions;
use std::collections::HashMap;

const SOURCE: &str = "namespace app\nverify captured_checked:\n    int64 seed = 7\n    function() returns int64 callback = function() returns int64: return seed\n    assert callback() == 7\nproperty captured_samples:\n    given repeat: bool\n    int64 seed = 7\n    function() returns int64 callback = function() returns int64: return seed\n    assert callback() == 7\n";
fn checked() -> Arc<CheckedResourceProgram> {
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(SOURCE, FileId::new(0)),
            HashMap::from([(FileId::new(0), SourceOrigin::Project)]),
            &[],
            CheckOptions::default(),
        )
        .unwrap(),
    )
}
fn blocks(checked: &CheckedResourceProgram) -> Vec<(&Block, crate::ExecutionPurpose)> {
    checked
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Verify(block) => Some((&block.body, crate::ExecutionPurpose::Verify)),
            Item::Property(block) => Some((&block.body, crate::ExecutionPurpose::Property)),
            _ => None,
        })
        .collect()
}
#[test]
fn original_test_capture_owner_rejects_cloned_foreign_wrong_purpose_and_namespace_blocks() {
    let checked = checked();
    let foreign = self::checked();
    for (body, purpose) in blocks(&checked) {
        assert!(OriginalClosureOwner::test_body(&checked, body, purpose, Some("app")).is_some());
        assert!(
            OriginalClosureOwner::test_body(&checked, &body.clone(), purpose, Some("app"))
                .is_none()
        );
        assert!(OriginalClosureOwner::test_body(&foreign, body, purpose, Some("app")).is_none());
        assert!(
            OriginalClosureOwner::test_body(
                &checked,
                body,
                crate::ExecutionPurpose::ReferenceRuntime,
                Some("app")
            )
            .is_none()
        );
        assert!(OriginalClosureOwner::test_body(&checked, body, purpose, Some("other")).is_none());
        assert!(OriginalClosureOwner::test_body(&checked, body, purpose, None).is_none());
    }
}
#[test]
fn original_test_capture_witness_preserves_original_body_context_and_capture_snapshot() {
    let checked = checked();
    for (body, purpose) in blocks(&checked) {
        let owner = OriginalClosureOwner::test_body(&checked, body, purpose, Some("app")).unwrap();
        let Stmt::VarDecl(callback) = &body.stmts[1] else {
            panic!("original callable")
        };
        let Expr::InlineFn(params, result, body, span) = &callback.value else {
            panic!("original closure")
        };
        let actual = checked.checked().type_map[span];
        let value = Value::Function {
            type_context: Box::new(ClosureTypeContext {
                return_type: result.clone(),
                ..ClosureTypeContext::default()
            }),
            params: params.clone(),
            body: body.clone(),
            captures: HashMap::from([("seed".into(), Value::Int64(7))]),
            capture_types: HashMap::new(),
            namespace_aliases: HashMap::new(),
            namespace: Some("app".into()),
        };
        let witness = CheckedClosureCapture::seal(&owner, &value).unwrap();
        assert!(witness.validate_source(&checked, actual, &value));
        for mutation in 0..3 {
            let mut changed = value.clone();
            let Value::Function {
                body,
                captures,
                type_context,
                ..
            } = &mut changed
            else {
                unreachable!()
            };
            match mutation {
                0 => body.stmts.clear(),
                1 => {
                    captures.insert("seed".into(), Value::Int64(8));
                }
                2 => type_context.return_type = None,
                _ => unreachable!(),
            }
            assert!(!witness.validate_source(&checked, actual, &changed));
        }
    }
}
