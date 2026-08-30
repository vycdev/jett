use jett_query::cache::{PARSE_FILE_ARTIFACT_KIND, ParseCacheKey, ParseCacheKeyRecord};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn parse_key_uses_the_canonical_v1_record_and_digest() {
    let key = ParseCacheKey::new(b"namespace app\n", [0xa5; 32], 7);

    assert_eq!(PARSE_FILE_ARTIFACT_KIND, "jett.parse-file.v1");
    assert_eq!(
        hex(key.record()),
        "6a6574742d63616368652d6b65790100000012000000000000006a6574742e70617273652d66696c652e7631010000002000000000000000a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5030000000100000008000000000000000e00000000000000020000002000000000000000d87e5710c3372e05ddedc6128aa8a32bbbf36629fe8104e916d4177c23592fcd03000000040000000000000007000000"
    );
    assert_eq!(
        hex(key.digest()),
        "d5099c3d2bfbc328f7e067810bd60d81f2ded61c2cdb289155058891a8d13e33"
    );
}

#[test]
fn parse_key_record_round_trips_current_inputs() {
    let source = b"namespace app\n";
    let compatibility_id = [0xa5; 32];
    let key = ParseCacheKey::new(source, compatibility_id, 7);

    let decoded =
        ParseCacheKeyRecord::decode(key.record()).expect("canonical record should decode");

    assert_eq!(decoded.compiler_compatibility_id(), &compatibility_id);
    assert_eq!(decoded.source_length(), source.len() as u64);
    assert_eq!(decoded.parser_policy(), 7);
    assert!(decoded.matches_source(source));
    assert!(!decoded.matches_source(b"namespace other\n"));
}

#[test]
fn parse_key_decoder_rejects_noncanonical_fixed_fields() {
    let canonical = ParseCacheKey::new(b"namespace app\n", [0xa5; 32], 7)
        .record()
        .to_vec();
    let mutations = [
        ("magic", 0, b'J'),
        ("format version", 14, 2),
        ("artifact kind", 26, b'X'),
        ("artifact schema", 44, 2),
        ("field count", 88, 2),
        ("first field tag", 92, 2),
    ];

    for (name, offset, value) in mutations {
        let mut record = canonical.clone();
        record[offset] = value;
        assert!(
            ParseCacheKeyRecord::decode(&record).is_err(),
            "decoder accepted noncanonical {name}"
        );
    }
}
