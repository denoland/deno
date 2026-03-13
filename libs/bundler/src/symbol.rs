// Copyright 2018-2026 the Deno authors. MIT license.

use crate::js::scope::DeclId;

/// A globally unique symbol identifier.
///
/// Identifies a declaration across all modules in the graph.
/// Consists of the module's dense index (from `BundlerGraph::specifier_to_index`)
/// and the declaration within that module (`DeclId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId {
  /// Dense module index from BundlerGraph.
  pub module: u32,
  /// The declaration within the module.
  pub decl: DeclId,
}

impl SymbolId {
  pub fn new(module: u32, decl: DeclId) -> Self {
    Self { module, decl }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_symbol_id_equality() {
    let sym1 = SymbolId::new(0, DeclId(0));
    let sym2 = SymbolId::new(0, DeclId(0));
    let sym3 = SymbolId::new(1, DeclId(0));

    assert_eq!(sym1, sym2);
    assert_ne!(sym1, sym3);
  }

  #[test]
  fn test_symbol_id_hash() {
    use rustc_hash::FxHashSet;

    let mut set = FxHashSet::default();
    let sym1 = SymbolId::new(0, DeclId(0));
    let sym2 = SymbolId::new(0, DeclId(0));

    set.insert(sym1);
    assert!(set.contains(&sym2));
  }
}
