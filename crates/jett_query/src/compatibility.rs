use sha2::{Digest, Sha256};
use std::fmt;

const INPUT_MAGIC: &[u8] = b"jett-compat-input";
const INPUT_FORMAT_VERSION: u32 = 1;
const COMPATIBILITY_MAGIC: &[u8] = b"jett-compiler-compat";
const COMPATIBILITY_FORMAT_VERSION: u32 = 1;
const PARSE_ARTIFACT_KIND: &[u8] = b"jett.parse-file.v1";
const PARSE_ARTIFACT_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CompatibilityDomain {
    CompilerSource = 1,
    LockedDependencies = 2,
    BundledStdlib = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityEntry {
    logical_path: String,
    content: Vec<u8>,
}

impl CompatibilityEntry {
    pub fn new(logical_path: impl Into<String>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            logical_path: logical_path.into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityInputRecord {
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl CompatibilityInputRecord {
    pub fn new(
        domain: CompatibilityDomain,
        mut entries: Vec<CompatibilityEntry>,
    ) -> Result<Self, CompatibilityError> {
        for entry in &entries {
            validate_logical_path(&entry.logical_path)?;
        }
        entries.sort_by(|left, right| {
            left.logical_path
                .as_bytes()
                .cmp(right.logical_path.as_bytes())
        });
        for entries in entries.windows(2) {
            if entries[0].logical_path == entries[1].logical_path {
                return Err(CompatibilityError::DuplicateLogicalPath(
                    entries[0].logical_path.clone(),
                ));
            }
        }

        let entry_count =
            u32::try_from(entries.len()).map_err(|_| CompatibilityError::EntryCountOverflow)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(INPUT_MAGIC);
        push_u32(&mut bytes, INPUT_FORMAT_VERSION);
        push_u32(&mut bytes, domain as u32);
        push_u32(&mut bytes, entry_count);
        for entry in entries {
            push_bytes(&mut bytes, entry.logical_path.as_bytes())?;
            push_bytes(&mut bytes, &entry.content)?;
        }
        let digest = Sha256::digest(&bytes).into();
        Ok(Self { bytes, digest })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerSourceIdentity {
    Revision(String),
    SourceTree([u8; 32]),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerCompatibilityId {
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl CompilerCompatibilityId {
    pub fn for_parse_file(
        package_version: &str,
        source_identity: CompilerSourceIdentity,
        locked_dependency_digest: [u8; 32],
        compiler_policy_revision: u32,
    ) -> Result<Self, CompatibilityError> {
        let source_identity = encode_source_identity(source_identity)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(COMPATIBILITY_MAGIC);
        push_u32(&mut bytes, COMPATIBILITY_FORMAT_VERSION);
        push_u32(&mut bytes, 6);
        push_component(&mut bytes, 1, package_version.as_bytes())?;
        push_component(&mut bytes, 2, &source_identity)?;
        push_component(&mut bytes, 3, &locked_dependency_digest)?;
        push_component(&mut bytes, 5, PARSE_ARTIFACT_KIND)?;
        push_component(&mut bytes, 6, &PARSE_ARTIFACT_SCHEMA.to_le_bytes())?;
        push_component(&mut bytes, 7, &compiler_policy_revision.to_le_bytes())?;
        let digest = Sha256::digest(&bytes).into();
        Ok(Self { bytes, digest })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityError {
    InvalidLogicalPath(String),
    DuplicateLogicalPath(String),
    EntryCountOverflow,
    LengthOverflow,
}

impl fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLogicalPath(path) => {
                write!(formatter, "invalid compatibility logical path: {path:?}")
            }
            Self::DuplicateLogicalPath(path) => {
                write!(formatter, "duplicate compatibility logical path: {path:?}")
            }
            Self::EntryCountOverflow => formatter.write_str("too many compatibility input entries"),
            Self::LengthOverflow => formatter.write_str("compatibility field is too large"),
        }
    }
}

impl std::error::Error for CompatibilityError {}

fn validate_logical_path(path: &str) -> Result<(), CompatibilityError> {
    let invalid = path.is_empty()
        || path.starts_with('/')
        || path.as_bytes().get(1) == Some(&b':')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."));
    if invalid {
        return Err(CompatibilityError::InvalidLogicalPath(path.to_string()));
    }
    Ok(())
}

fn encode_source_identity(
    source_identity: CompilerSourceIdentity,
) -> Result<Vec<u8>, CompatibilityError> {
    match source_identity {
        CompilerSourceIdentity::Revision(revision) => {
            let mut bytes = vec![1];
            push_bytes(&mut bytes, revision.as_bytes())?;
            Ok(bytes)
        }
        CompilerSourceIdentity::SourceTree(digest) => {
            let mut bytes = Vec::with_capacity(33);
            bytes.push(2);
            bytes.extend_from_slice(&digest);
            Ok(bytes)
        }
    }
}

fn push_component(record: &mut Vec<u8>, tag: u32, value: &[u8]) -> Result<(), CompatibilityError> {
    push_u32(record, tag);
    push_bytes(record, value)
}

fn push_bytes(record: &mut Vec<u8>, value: &[u8]) -> Result<(), CompatibilityError> {
    let length = u64::try_from(value.len()).map_err(|_| CompatibilityError::LengthOverflow)?;
    record.extend_from_slice(&length.to_le_bytes());
    record.extend_from_slice(value);
    Ok(())
}

fn push_u32(record: &mut Vec<u8>, value: u32) {
    record.extend_from_slice(&value.to_le_bytes());
}
