use std::collections::HashMap;

/// An interned string identifier. Comparisons are O(1) integer comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(u32);

impl Symbol {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// String interner that deduplicates identifiers.
/// All identifiers in the compiler are interned through this.
#[derive(Debug, Default)]
pub struct SymbolInterner {
    map: HashMap<String, Symbol>,
    strings: Vec<String>,
}

impl SymbolInterner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a string, returning its Symbol handle.
    /// If the string was already interned, returns the existing Symbol.
    pub fn intern(&mut self, s: &str) -> Symbol {
        if let Some(&sym) = self.map.get(s) {
            return sym;
        }
        let sym = Symbol(self.strings.len() as u32);
        self.strings.push(s.to_owned());
        self.map.insert(s.to_owned(), sym);
        sym
    }

    /// Look up the string for a given Symbol.
    pub fn resolve(&self, sym: Symbol) -> &str {
        &self.strings[sym.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_same_symbol_for_same_string() {
        let mut interner = SymbolInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("hello");
        assert_eq!(a, b);
    }

    #[test]
    fn intern_returns_different_symbols_for_different_strings() {
        let mut interner = SymbolInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("world");
        assert_ne!(a, b);
    }

    #[test]
    fn resolve_returns_original_string() {
        let mut interner = SymbolInterner::new();
        let sym = interner.intern("jett");
        assert_eq!(interner.resolve(sym), "jett");
    }
}
