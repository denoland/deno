// Copyright 2018-2026 the Deno authors. MIT license.

use rustc_hash::FxHashMap;

use crate::symbol::SymbolId;

/// Union-find structure for cross-module symbol resolution.
///
/// Each equivalence class contains all SymbolIds that resolve to the
/// same underlying declaration. The representative (root) of each
/// class is the actual declaration's SymbolId.
///
/// Built once after graph construction, then queried with `&self`
/// via the flattened `parent` map (no interior mutability needed).
#[derive(Debug, Clone)]
pub struct SymbolUnionFind {
  /// Maps each symbol to its representative (root) symbol.
  /// After `flatten()`, every entry points directly to the root.
  /// Symbols not in the map are their own representative.
  parent: FxHashMap<SymbolId, SymbolId>,
}

impl SymbolUnionFind {
  /// Create an empty union-find.
  pub fn empty() -> Self {
    Self {
      parent: FxHashMap::default(),
    }
  }

  /// Union two symbols into the same equivalence class.
  /// The `decl` symbol (the declaration site) is preferred as the root.
  pub fn union(&mut self, import_sym: SymbolId, decl_sym: SymbolId) {
    let root_import = self.find_mut(import_sym);
    let root_decl = self.find_mut(decl_sym);
    if root_import != root_decl {
      // Always make the declaration the root.
      self.parent.insert(root_import, root_decl);
    }
  }

  /// Find with path compression (mutable version for build phase).
  fn find_mut(&mut self, mut sym: SymbolId) -> SymbolId {
    let mut path = Vec::new();
    while let Some(&p) = self.parent.get(&sym) {
      if p == sym {
        break;
      }
      path.push(sym);
      sym = p;
    }
    // Path compression: point all visited nodes directly to root.
    for s in path {
      self.parent.insert(s, sym);
    }
    sym
  }

  /// Flatten all paths so every entry points directly to its root.
  /// After this, `find()` is a single hash lookup with no chasing.
  pub fn flatten(&mut self) {
    let keys: Vec<SymbolId> = self.parent.keys().copied().collect();
    for key in keys {
      let root = self.find_mut(key);
      self.parent.insert(key, root);
    }
    // Remove self-loops (entries where parent == self).
    self.parent.retain(|k, v| k != v);
  }

  /// Look up the representative (root) symbol for a given symbol.
  ///
  /// Returns `sym` itself if it is not in the union-find (i.e., it IS
  /// the declaration, or it's unresolved/external).
  #[inline]
  pub fn find(&self, sym: SymbolId) -> SymbolId {
    self.parent.get(&sym).copied().unwrap_or(sym)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::js::scope::DeclId;

  fn sym(module: u32, decl: u32) -> SymbolId {
    SymbolId::new(module, DeclId(decl))
  }

  #[test]
  fn test_find_self() {
    let uf = SymbolUnionFind::empty();
    let s = sym(0, 0);
    assert_eq!(uf.find(s), s);
  }

  #[test]
  fn test_simple_union() {
    let mut uf = SymbolUnionFind::empty();
    let import = sym(0, 0);
    let decl = sym(1, 0);

    uf.union(import, decl);
    uf.flatten();

    assert_eq!(uf.find(import), decl);
    assert_eq!(uf.find(decl), decl);
  }

  #[test]
  fn test_chain_union() {
    // A imports from B, B re-exports from C.
    let mut uf = SymbolUnionFind::empty();
    let a = sym(0, 0);
    let b = sym(1, 0);
    let c = sym(2, 0);

    uf.union(b, c); // B's re-export → C's declaration
    uf.union(a, b); // A's import → B's re-export

    uf.flatten();

    assert_eq!(uf.find(a), c);
    assert_eq!(uf.find(b), c);
    assert_eq!(uf.find(c), c);
  }

  #[test]
  fn test_flatten() {
    let mut uf = SymbolUnionFind::empty();
    let a = sym(0, 0);
    let b = sym(1, 0);
    let c = sym(2, 0);

    uf.parent.insert(a, b);
    uf.parent.insert(b, c);

    uf.flatten();

    assert_eq!(uf.find(a), c);
    assert_eq!(uf.find(b), c);
  }

  #[test]
  fn test_separate_classes() {
    let mut uf = SymbolUnionFind::empty();
    let a1 = sym(0, 0);
    let d1 = sym(1, 0);
    let a2 = sym(2, 0);
    let d2 = sym(3, 0);

    uf.union(a1, d1);
    uf.union(a2, d2);
    uf.flatten();

    assert_eq!(uf.find(a1), d1);
    assert_eq!(uf.find(a2), d2);
    assert_ne!(uf.find(a1), uf.find(a2));
  }

  #[test]
  fn test_multiple_imports_same_decl() {
    let mut uf = SymbolUnionFind::empty();
    let import1 = sym(0, 0);
    let import2 = sym(1, 0);
    let import3 = sym(2, 0);
    let decl = sym(3, 0);

    uf.union(import1, decl);
    uf.union(import2, decl);
    uf.union(import3, decl);
    uf.flatten();

    assert_eq!(uf.find(import1), decl);
    assert_eq!(uf.find(import2), decl);
    assert_eq!(uf.find(import3), decl);
  }

  #[test]
  fn test_diamond_reexport() {
    // Diamond: A and B both re-export from C, D imports from both A and B.
    let mut uf = SymbolUnionFind::empty();
    let c_decl = sym(2, 0);
    let a_reexport = sym(0, 0);
    let b_reexport = sym(1, 0);
    let d_import_a = sym(3, 0);
    let d_import_b = sym(3, 1);

    uf.union(a_reexport, c_decl);
    uf.union(b_reexport, c_decl);
    uf.union(d_import_a, a_reexport);
    uf.union(d_import_b, b_reexport);
    uf.flatten();

    assert_eq!(uf.find(d_import_a), c_decl);
    assert_eq!(uf.find(d_import_b), c_decl);
    assert_eq!(uf.find(a_reexport), c_decl);
    assert_eq!(uf.find(b_reexport), c_decl);
  }
}
