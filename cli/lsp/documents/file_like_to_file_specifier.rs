// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::url;
use deno_core::ModuleSpecifier;
use tower_lsp::lsp_types::Url;

/// Convert a e.g. `deno-notebook-cell:` specifier to a `file:` specifier.
/// ```rust
/// assert_eq!(
///   file_like_to_file_specifier(
///     &Url::parse("deno-notebook-cell:/path/to/file.ipynb#abc").unwrap(),
///   ),
///   Some(Url::parse("file:///path/to/file.ipynb#abc").unwrap()),
/// );
pub fn file_like_to_file_specifier(specifier: &Url) -> Option<Url> {
  if matches!(specifier.scheme(), "untitled" | "deno-notebook-cell") {
    if let Ok(mut s) = ModuleSpecifier::parse(&format!(
      "file://{}",
      &specifier.as_str()
        [url::quirks::internal_components(specifier).host_end as usize..],
    )) {
      s.query_pairs_mut()
        .append_pair("scheme", specifier.scheme());
      return Some(s);
    }
  }
  None
}
