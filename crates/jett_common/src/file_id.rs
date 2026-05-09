/// Unique identifier for a source file in the compilation.
/// Used as an index into the project's file table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(u32);

/// File ids at or above this value are reserved for compiler-shipped stdlib.
pub const STDLIB_FILE_ID_START: u32 = 10_000;

impl FileId {
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    pub fn index(self) -> u32 {
        self.0
    }

    pub fn is_stdlib(self) -> bool {
        self.0 >= STDLIB_FILE_ID_START
    }
}
