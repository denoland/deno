// Copyright 2018-2026 the Deno authors. MIT license.

/// HMR metadata extracted from `import.meta.hot` calls in a module.
#[derive(Debug, Clone, Default)]
pub struct HmrInfo {
  /// Module calls `import.meta.hot.accept()` with no deps (self-accept).
  pub self_accepts: bool,
  /// Module calls `import.meta.hot.accept('./dep', cb)` with specific dep
  /// specifiers.
  pub accepted_deps: Vec<String>,
  /// Module calls `import.meta.hot.dispose(cb)`.
  pub has_dispose: bool,
  /// Module calls `import.meta.hot.decline()`.
  pub declines: bool,
  /// Module references `import.meta.hot` at all.
  pub has_hot_api: bool,
}
