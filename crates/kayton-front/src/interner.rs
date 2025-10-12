use indexmap::IndexSet;
use smol_str::SmolStr;
use std::hash::{BuildHasherDefault, Hash};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(u32);

impl Symbol {
    pub fn new(raw: u32) -> Self {
        Symbol(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Default, Clone)]
pub struct SymbolInterner {
    strings: IndexSet<SmolStr, BuildHasherDefault<rustc_hash::FxHasher>>,
}

impl SymbolInterner {
    pub fn new() -> Self {
        Self {
            strings: IndexSet::with_hasher(BuildHasherDefault::default()),
        }
    }

    pub fn intern(&mut self, value: impl Into<SmolStr>) -> Symbol {
        let value = value.into();
        if let Some(idx) = self.strings.get_index_of(&value) {
            Symbol(idx as u32)
        } else {
            let (idx, _) = self.strings.insert_full(value);
            Symbol(idx as u32)
        }
    }

    pub fn resolve(&self, symbol: Symbol) -> Option<&SmolStr> {
        self.strings.get_index(symbol.raw() as usize)
    }
}
