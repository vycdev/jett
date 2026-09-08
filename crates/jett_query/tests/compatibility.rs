use jett_query::compatibility::{
    CompatibilityDomain, CompatibilityEntry, CompatibilityError, CompatibilityInputRecord,
    CompilerCompatibilityId, CompilerSourceIdentity,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn compiler_input() -> CompatibilityInputRecord {
    CompatibilityInputRecord::new(
        CompatibilityDomain::CompilerSource,
        vec![
            CompatibilityEntry::new("src/parser.rs", b"parse".to_vec()),
            CompatibilityEntry::new("src/lexer.rs", b"lex".to_vec()),
        ],
    )
    .unwrap()
}

#[test]
fn compatibility_input_record_has_canonical_golden_encoding() {
    let record = compiler_input();

    assert_eq!(
        hex(record.bytes()),
        "6a6574742d636f6d7061742d696e7075740100000001000000020000000c000000000000007372632f6c657865722e727303000000000000006c65780d000000000000007372632f7061727365722e727305000000000000007061727365"
    );
    assert_eq!(
        hex(record.digest()),
        "a06aa5eea8ed673123fecd3fd2a7f750894d04c9ff0af17d83448514a997a0db"
    );
}

#[test]
fn compatibility_input_record_sorts_entries_and_rejects_noncanonical_paths() {
    let first = compiler_input();
    let reordered = CompatibilityInputRecord::new(
        CompatibilityDomain::CompilerSource,
        vec![
            CompatibilityEntry::new("src/lexer.rs", b"lex".to_vec()),
            CompatibilityEntry::new("src/parser.rs", b"parse".to_vec()),
        ],
    )
    .unwrap();
    assert_eq!(first, reordered);

    for path in [
        "",
        "/src/main.rs",
        "C:/src/main.rs",
        "src\\main.rs",
        "src//main.rs",
        "src/./main.rs",
        "src/../main.rs",
        "src/\0main.rs",
    ] {
        assert_eq!(
            CompatibilityInputRecord::new(
                CompatibilityDomain::CompilerSource,
                vec![CompatibilityEntry::new(path, Vec::new())],
            ),
            Err(CompatibilityError::InvalidLogicalPath(path.to_string()))
        );
    }

    assert_eq!(
        CompatibilityInputRecord::new(
            CompatibilityDomain::CompilerSource,
            vec![
                CompatibilityEntry::new("src/main.rs", b"one".to_vec()),
                CompatibilityEntry::new("src/main.rs", b"two".to_vec()),
            ],
        ),
        Err(CompatibilityError::DuplicateLogicalPath(
            "src/main.rs".to_string()
        ))
    );
}

#[test]
fn parse_compiler_compatibility_id_has_canonical_golden_encoding() {
    let identity = CompilerCompatibilityId::for_parse_file(
        "0.1.0",
        CompilerSourceIdentity::SourceTree(*compiler_input().digest()),
        [0x11; 32],
        7,
    )
    .unwrap();

    assert_eq!(
        hex(identity.bytes()),
        "6a6574742d636f6d70696c65722d636f6d7061740100000006000000010000000500000000000000302e312e3002000000210000000000000002a06aa5eea8ed673123fecd3fd2a7f750894d04c9ff0af17d83448514a997a0db03000000200000000000000011111111111111111111111111111111111111111111111111111111111111110500000012000000000000006a6574742e70617273652d66696c652e76310600000004000000000000000100000007000000040000000000000007000000"
    );
    assert_eq!(
        hex(identity.digest()),
        "bb17b8dd58181171afee8b6567be50eee8e28b03b8bbad49d6f46a3e15299c0a"
    );
}

#[test]
fn compiler_compatibility_id_changes_with_every_semantic_component() {
    let source = CompilerSourceIdentity::Revision("abc123".to_string());
    let baseline =
        CompilerCompatibilityId::for_parse_file("0.1.0", source.clone(), [3; 32], 1).unwrap();

    let changed = [
        CompilerCompatibilityId::for_parse_file("0.1.1", source.clone(), [3; 32], 1).unwrap(),
        CompilerCompatibilityId::for_parse_file(
            "0.1.0",
            CompilerSourceIdentity::Revision("def456".to_string()),
            [3; 32],
            1,
        )
        .unwrap(),
        CompilerCompatibilityId::for_parse_file("0.1.0", source.clone(), [4; 32], 1).unwrap(),
        CompilerCompatibilityId::for_parse_file("0.1.0", source, [3; 32], 2).unwrap(),
    ];

    assert!(
        changed
            .iter()
            .all(|candidate| candidate.digest() != baseline.digest())
    );
}
