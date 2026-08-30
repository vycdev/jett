/// The authority that supplied a source file.
///
/// This lives in `jett_common` because source authority must survive through
/// resolution and every lowering phase; it is not owned by the query engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceOrigin {
    Project,
    Dependency(String),
    Stdlib,
}
