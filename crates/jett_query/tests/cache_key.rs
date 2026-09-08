use jett_query::cache::{
    CacheEnvelope, PARSE_FILE_ARTIFACT_KIND, ParseCacheKey, ParseCacheKeyRecord,
};

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

#[test]
fn cache_envelope_round_trips_an_authenticated_parse_payload() {
    let key = ParseCacheKey::new(b"namespace app\n", [0xa5; 32], 7);
    let authentication_key = [0x5a; 32];
    let payload = b"canonical parsed-file payload";

    let encoded = CacheEnvelope::encode(&key, payload, &authentication_key)
        .expect("bounded payload should encode");
    assert_eq!(encoded.len(), 364);
    assert_eq!(
        hex(&encoded[encoded.len() - 32..]),
        "d598468a57d2ae7e012d84e7b42efe5a0fbf6d0d48d948f82b61198c2b6263ec"
    );
    let decoded = CacheEnvelope::decode(&encoded, &key, &authentication_key)
        .expect("canonical envelope should decode");

    assert_eq!(decoded.payload(), payload);
    assert_eq!(decoded.key_record(), key.record());
}

#[test]
fn cache_envelope_rejects_tampering_and_the_wrong_key() {
    let key = ParseCacheKey::new(b"namespace app\n", [0xa5; 32], 7);
    let authentication_key = [0x5a; 32];
    let mut encoded = CacheEnvelope::encode(&key, b"payload", &authentication_key).unwrap();
    let payload_byte = encoded.len() - 33;
    encoded[payload_byte] ^= 1;

    assert!(CacheEnvelope::decode(&encoded, &key, &authentication_key).is_err());

    let encoded = CacheEnvelope::encode(&key, b"payload", &authentication_key).unwrap();
    let different_key = ParseCacheKey::new(b"namespace other\n", [0xa5; 32], 7);
    assert!(CacheEnvelope::decode(&encoded, &different_key, &authentication_key).is_err());
    assert!(CacheEnvelope::decode(&encoded, &key, &[0x33; 32]).is_err());
}

#[test]
fn cache_envelope_rejects_oversized_input_before_authentication() {
    let key = ParseCacheKey::new(b"namespace app\n", [0xa5; 32], 7);
    let oversized = vec![0; 64 * 1024 * 1024 + 4097];

    let error = CacheEnvelope::decode(&oversized, &key, &[0x5a; 32])
        .expect_err("oversized input must be rejected");

    assert_eq!(error.to_string(), "cache envelope exceeds size limit");
}
