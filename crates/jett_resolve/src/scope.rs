use jett_common::Span;
use std::collections::HashMap;

/// Uniquely identifies a definition within the resolve pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefId(u32);

impl DefId {
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    pub fn index(self) -> u32 {
        self.0
    }
}

/// Uniquely identifies a scope within the resolve pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(u32);

impl ScopeId {
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    pub fn index(self) -> u32 {
        self.0
    }
}

/// The kind of entity a definition represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Function,
    Struct,
    Enum,
    Machine,
    Variable,
    Param,
    Constant,
    Namespace,
}

/// Metadata about a single definition.
#[derive(Debug, Clone)]
pub struct DefInfo {
    pub id: DefId,
    pub name: String,
    pub kind: DefKind,
    pub span: Span,
}

/// A single lexical scope containing bindings and an optional parent.
#[derive(Debug, Clone)]
pub struct Scope {
    pub bindings: HashMap<String, DefId>,
    pub parent: Option<ScopeId>,
}

/// The complete table of scopes and definitions produced by name resolution.
#[derive(Debug)]
pub struct ScopeTable {
    pub scopes: Vec<Scope>,
    pub definitions: Vec<DefInfo>,
}

impl ScopeTable {
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            definitions: Vec::new(),
        }
    }

    /// Create a new scope, optionally nested under `parent`.
    pub fn new_scope(&mut self, parent: Option<ScopeId>) -> ScopeId {
        let id = ScopeId::new(self.scopes.len() as u32);
        self.scopes.push(Scope {
            bindings: HashMap::new(),
            parent,
        });
        id
    }

    /// Register a new definition and return its `DefId`.
    pub fn new_def(&mut self, name: String, kind: DefKind, span: Span) -> DefId {
        let id = DefId::new(self.definitions.len() as u32);
        self.definitions.push(DefInfo {
            id,
            name,
            kind,
            span,
        });
        id
    }

    /// Insert a binding into the given scope. Returns the previous `DefId` if
    /// the name was already bound in this exact scope (duplicate definition).
    pub fn bind(&mut self, scope: ScopeId, name: String, def: DefId) -> Option<DefId> {
        self.scopes[scope.index() as usize]
            .bindings
            .insert(name, def)
    }

    /// Look up a name starting from `scope`, walking up through parent scopes.
    pub fn lookup(&self, scope: ScopeId, name: &str) -> Option<DefId> {
        let s = &self.scopes[scope.index() as usize];
        if let Some(&def) = s.bindings.get(name) {
            return Some(def);
        }
        if let Some(parent) = s.parent {
            return self.lookup(parent, name);
        }
        None
    }

    /// Check whether `name` is bound in the **immediate** scope (not parents).
    pub fn lookup_local(&self, scope: ScopeId, name: &str) -> Option<DefId> {
        self.scopes[scope.index() as usize]
            .bindings
            .get(name)
            .copied()
    }

    /// Check whether `name` is bound in any **ancestor** scope (excluding the
    /// given scope itself). Used for shadowing detection.
    pub fn lookup_ancestor(&self, scope: ScopeId, name: &str) -> Option<DefId> {
        let s = &self.scopes[scope.index() as usize];
        if let Some(parent) = s.parent {
            return self.lookup(parent, name);
        }
        None
    }

    /// Get the definition info for a `DefId`.
    pub fn def(&self, id: DefId) -> &DefInfo {
        &self.definitions[id.index() as usize]
    }
}

impl Default for ScopeTable {
    fn default() -> Self {
        Self::new()
    }
}
