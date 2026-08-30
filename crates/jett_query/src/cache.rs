use sha2::{Digest, Sha256};

const KEY_MAGIC: &[u8] = b"jett-cache-key";
const KEY_FORMAT_VERSION: u32 = 1;
const PARSE_ARTIFACT_SCHEMA: u32 = 1;
const SOURCE_LENGTH_TAG: u32 = 1;
const SOURCE_DIGEST_TAG: u32 = 2;
const PARSER_POLICY_TAG: u32 = 3;

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

impl std::fmt::Display for CacheKeyDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for CacheKeyDecodeError {}

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
