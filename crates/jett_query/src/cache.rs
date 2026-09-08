use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const KEY_MAGIC: &[u8] = b"jett-cache-key";
const KEY_FORMAT_VERSION: u32 = 1;
const PARSE_ARTIFACT_SCHEMA: u32 = 1;
const SOURCE_LENGTH_TAG: u32 = 1;
const SOURCE_DIGEST_TAG: u32 = 2;
const PARSER_POLICY_TAG: u32 = 3;
const ENVELOPE_MAGIC: &[u8] = b"JETTCACHE";
const ENVELOPE_VERSION: u32 = 1;
const AUTHENTICATION_DOMAIN: &[u8] = b"jett-cache-object-auth-v1";
const MAX_PAYLOAD_LENGTH: usize = 64 * 1024 * 1024;
const MAX_ENVELOPE_LENGTH: usize = MAX_PAYLOAD_LENGTH + 4096;

pub const PARSE_FILE_ARTIFACT_KIND: &str = "jett.parse-file.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseCacheKey {
    record: Vec<u8>,
    digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseCacheKeyRecord {
    compiler_compatibility_id: [u8; 32],
    source_length: u64,
    source_digest: [u8; 32],
    parser_policy: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKeyDecodeError(&'static str);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEnvelope {
    key_record: Vec<u8>,
    payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEnvelopeError(&'static str);

impl std::fmt::Display for CacheKeyDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for CacheKeyDecodeError {}

impl std::fmt::Display for CacheEnvelopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for CacheEnvelopeError {}

impl CacheEnvelope {
    pub fn encode(
        key: &ParseCacheKey,
        payload: &[u8],
        authentication_key: &[u8; 32],
    ) -> Result<Vec<u8>, CacheEnvelopeError> {
        if payload.len() > MAX_PAYLOAD_LENGTH {
            return Err(CacheEnvelopeError("cache payload exceeds 64 MiB"));
        }

        let payload_length = payload.len() as u64;
        let payload_digest = Sha256::digest(payload);
        let mut envelope = Vec::with_capacity(
            ENVELOPE_MAGIC.len()
                + 4
                + 8
                + PARSE_FILE_ARTIFACT_KIND.len()
                + 4
                + 32
                + 8
                + key.record().len()
                + 8
                + 8
                + 32
                + payload.len()
                + 32,
        );
        envelope.extend_from_slice(ENVELOPE_MAGIC);
        push_u32(&mut envelope, ENVELOPE_VERSION);
        push_bytes(&mut envelope, PARSE_FILE_ARTIFACT_KIND.as_bytes());
        push_u32(&mut envelope, PARSE_ARTIFACT_SCHEMA);
        envelope.extend_from_slice(key.digest());
        push_bytes(&mut envelope, key.record());
        envelope.extend_from_slice(&payload_length.to_le_bytes());
        envelope.extend_from_slice(&payload_length.to_le_bytes());
        envelope.extend_from_slice(&payload_digest);
        envelope.extend_from_slice(payload);
        let authenticator = hmac_sha256(authentication_key, &envelope);
        envelope.extend_from_slice(&authenticator);
        Ok(envelope)
    }

    pub fn decode(
        envelope: &[u8],
        requested_key: &ParseCacheKey,
        authentication_key: &[u8; 32],
    ) -> Result<Self, CacheEnvelopeError> {
        if envelope.len() > MAX_ENVELOPE_LENGTH {
            return Err(CacheEnvelopeError("cache envelope exceeds size limit"));
        }
        if envelope.len() < 32 {
            return Err(CacheEnvelopeError("truncated cache envelope"));
        }
        let (authenticated, stored_authenticator) = envelope.split_at(envelope.len() - 32);
        let expected_authenticator = hmac_sha256(authentication_key, authenticated);
        if !constant_time_equal(stored_authenticator, &expected_authenticator) {
            return Err(CacheEnvelopeError("invalid cache-envelope authenticator"));
        }

        let mut reader = EnvelopeReader::new(authenticated);
        if reader.take(ENVELOPE_MAGIC.len())? != ENVELOPE_MAGIC {
            return Err(CacheEnvelopeError("invalid cache-envelope magic"));
        }
        if reader.read_u32()? != ENVELOPE_VERSION {
            return Err(CacheEnvelopeError("unsupported cache-envelope version"));
        }
        if reader.read_bytes()? != PARSE_FILE_ARTIFACT_KIND.as_bytes() {
            return Err(CacheEnvelopeError(
                "unexpected cache-envelope artifact kind",
            ));
        }
        if reader.read_u32()? != PARSE_ARTIFACT_SCHEMA {
            return Err(CacheEnvelopeError("unsupported cache-envelope schema"));
        }
        if reader.take(32)? != requested_key.digest() {
            return Err(CacheEnvelopeError("cache-envelope key digest mismatch"));
        }
        let key_record = reader.read_bytes()?;
        ParseCacheKeyRecord::decode(key_record)
            .map_err(|_| CacheEnvelopeError("invalid cache-envelope key record"))?;
        if key_record != requested_key.record()
            || Sha256::digest(key_record).as_slice() != requested_key.digest()
        {
            return Err(CacheEnvelopeError("cache-envelope key record mismatch"));
        }

        let uncompressed_length = reader.read_u64()?;
        let stored_length = reader.read_u64()?;
        if uncompressed_length != stored_length {
            return Err(CacheEnvelopeError(
                "compressed cache payload is unsupported",
            ));
        }
        let payload_length = usize::try_from(stored_length)
            .map_err(|_| CacheEnvelopeError("cache payload length overflows usize"))?;
        if payload_length > MAX_PAYLOAD_LENGTH {
            return Err(CacheEnvelopeError("cache payload exceeds 64 MiB"));
        }
        let payload_digest = reader.take(32)?;
        let payload = reader.take(payload_length)?;
        if !reader.is_empty() {
            return Err(CacheEnvelopeError("trailing cache-envelope bytes"));
        }
        if Sha256::digest(payload).as_slice() != payload_digest {
            return Err(CacheEnvelopeError("cache payload digest mismatch"));
        }

        Ok(Self {
            key_record: key_record.to_vec(),
            payload: payload.to_vec(),
        })
    }

    pub fn key_record(&self) -> &[u8] {
        &self.key_record
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl ParseCacheKey {
    pub fn new(source: &[u8], compiler_compatibility_id: [u8; 32], parser_policy: u32) -> Self {
        let source_digest = Sha256::digest(source);
        let mut record = Vec::with_capacity(172);
        record.extend_from_slice(KEY_MAGIC);
        push_u32(&mut record, KEY_FORMAT_VERSION);
        push_bytes(&mut record, PARSE_FILE_ARTIFACT_KIND.as_bytes());
        push_u32(&mut record, PARSE_ARTIFACT_SCHEMA);
        push_bytes(&mut record, &compiler_compatibility_id);
        push_u32(&mut record, 3);
        push_field(
            &mut record,
            SOURCE_LENGTH_TAG,
            &(source.len() as u64).to_le_bytes(),
        );
        push_field(&mut record, SOURCE_DIGEST_TAG, &source_digest);
        push_field(&mut record, PARSER_POLICY_TAG, &parser_policy.to_le_bytes());
        let digest = Sha256::digest(&record).into();
        Self { record, digest }
    }

    pub fn record(&self) -> &[u8] {
        &self.record
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

impl ParseCacheKeyRecord {
    pub fn decode(record: &[u8]) -> Result<Self, CacheKeyDecodeError> {
        let mut reader = RecordReader::new(record);
        if reader.take(KEY_MAGIC.len())? != KEY_MAGIC {
            return Err(CacheKeyDecodeError("invalid cache-key magic"));
        }
        if reader.read_u32()? != KEY_FORMAT_VERSION {
            return Err(CacheKeyDecodeError("unsupported cache-key format"));
        }
        if reader.read_bytes()? != PARSE_FILE_ARTIFACT_KIND.as_bytes() {
            return Err(CacheKeyDecodeError("unexpected cache artifact kind"));
        }
        if reader.read_u32()? != PARSE_ARTIFACT_SCHEMA {
            return Err(CacheKeyDecodeError("unsupported parse artifact schema"));
        }
        let compiler_compatibility_id = to_array(reader.read_bytes()?, "invalid compatibility ID")?;
        if reader.read_u32()? != 3 {
            return Err(CacheKeyDecodeError("invalid parse-key field count"));
        }

        if reader.read_u32()? != SOURCE_LENGTH_TAG {
            return Err(CacheKeyDecodeError("invalid source-length tag"));
        }
        let source_length = u64::from_le_bytes(to_array(
            reader.read_bytes()?,
            "invalid source-length field",
        )?);
        if reader.read_u32()? != SOURCE_DIGEST_TAG {
            return Err(CacheKeyDecodeError("invalid source-digest tag"));
        }
        let source_digest = to_array(reader.read_bytes()?, "invalid source-digest field")?;
        if reader.read_u32()? != PARSER_POLICY_TAG {
            return Err(CacheKeyDecodeError("invalid parser-policy tag"));
        }
        let parser_policy = u32::from_le_bytes(to_array(
            reader.read_bytes()?,
            "invalid parser-policy field",
        )?);
        if !reader.is_empty() {
            return Err(CacheKeyDecodeError("trailing cache-key bytes"));
        }
        Ok(Self {
            compiler_compatibility_id,
            source_length,
            source_digest,
            parser_policy,
        })
    }

    pub fn compiler_compatibility_id(&self) -> &[u8; 32] {
        &self.compiler_compatibility_id
    }

    pub fn source_length(&self) -> u64 {
        self.source_length
    }

    pub fn parser_policy(&self) -> u32 {
        self.parser_policy
    }

    pub fn matches_source(&self, source: &[u8]) -> bool {
        self.source_length == source.len() as u64
            && self.source_digest.as_slice() == Sha256::digest(source).as_slice()
    }
}

struct RecordReader<'a> {
    remaining: &'a [u8],
}

struct EnvelopeReader<'a> {
    remaining: &'a [u8],
}

impl<'a> EnvelopeReader<'a> {
    fn new(envelope: &'a [u8]) -> Self {
        Self {
            remaining: envelope,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CacheEnvelopeError> {
        if length > self.remaining.len() {
            return Err(CacheEnvelopeError("truncated cache envelope"));
        }
        let (value, remaining) = self.remaining.split_at(length);
        self.remaining = remaining;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, CacheEnvelopeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(
            |_| CacheEnvelopeError("truncated cache-envelope u32"),
        )?))
    }

    fn read_u64(&mut self) -> Result<u64, CacheEnvelopeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(
            |_| CacheEnvelopeError("truncated cache-envelope u64"),
        )?))
    }

    fn read_bytes(&mut self) -> Result<&'a [u8], CacheEnvelopeError> {
        let length = usize::try_from(self.read_u64()?)
            .map_err(|_| CacheEnvelopeError("cache-envelope field is too large"))?;
        self.take(length)
    }

    fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

impl<'a> RecordReader<'a> {
    fn new(record: &'a [u8]) -> Self {
        Self { remaining: record }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CacheKeyDecodeError> {
        if length > self.remaining.len() {
            return Err(CacheKeyDecodeError("truncated cache-key record"));
        }
        let (value, remaining) = self.remaining.split_at(length);
        self.remaining = remaining;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, CacheKeyDecodeError> {
        Ok(u32::from_le_bytes(to_array(
            self.take(4)?,
            "truncated u32",
        )?))
    }

    fn read_u64(&mut self) -> Result<u64, CacheKeyDecodeError> {
        Ok(u64::from_le_bytes(to_array(
            self.take(8)?,
            "truncated u64",
        )?))
    }

    fn read_bytes(&mut self) -> Result<&'a [u8], CacheKeyDecodeError> {
        let length = usize::try_from(self.read_u64()?)
            .map_err(|_| CacheKeyDecodeError("cache-key field is too large"))?;
        self.take(length)
    }

    fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

fn to_array<const N: usize>(
    value: &[u8],
    message: &'static str,
) -> Result<[u8; N], CacheKeyDecodeError> {
    value.try_into().map_err(|_| CacheKeyDecodeError(message))
}

fn push_field(record: &mut Vec<u8>, tag: u32, value: &[u8]) {
    push_u32(record, tag);
    push_bytes(record, value);
}

fn push_bytes(record: &mut Vec<u8>, value: &[u8]) {
    record.extend_from_slice(&(value.len() as u64).to_le_bytes());
    record.extend_from_slice(value);
}

fn push_u32(record: &mut Vec<u8>, value: u32) {
    record.extend_from_slice(&value.to_le_bytes());
}

fn hmac_sha256(key: &[u8; 32], envelope: &[u8]) -> [u8; 32] {
    let mut authentication =
        Hmac::<Sha256>::new_from_slice(key).expect("HMAC-SHA-256 accepts a fixed 32-byte key");
    authentication.update(AUTHENTICATION_DOMAIN);
    authentication.update(envelope);
    authentication.finalize().into_bytes().into()
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.ct_eq(right).into()
}
