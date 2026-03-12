// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::ModuleSpecifier;
use deno_ast::SourceRange;

/// The kind of import relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportKind {
  /// Static ESM import: `import x from './mod'`
  Import,
  /// Dynamic import: `import('./mod')`
  DynamicImport,
  /// CommonJS require: `require('./mod')`
  Require,
  /// CSS @import: `@import './other.css'`
  CssImport,
  /// CSS url() reference: `background: url('./image.png')`
  CssUrl,
  /// HTML <script src="...">
  HtmlScript,
  /// HTML <link rel="stylesheet" href="...">
  HtmlLink,
  /// HTML asset reference: `<img src="...">`, etc.
  HtmlAsset,
  /// Cross-environment URL reference.
  UrlReference,
}

/// A resolved dependency edge in the module graph.
#[derive(Debug, Clone)]
pub struct Dependency {
  /// The import specifier as written in source code.
  pub specifier: String,
  /// The resolved target module.
  pub resolved: ModuleSpecifier,
  /// The kind of import.
  pub kind: ImportKind,
  /// The range of the specifier in the importer's source.
  pub range: SourceRange,
}
