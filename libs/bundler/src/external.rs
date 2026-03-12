// Copyright 2018-2026 the Deno authors. MIT license.

/// Node.js built-in module names.
const NODE_BUILTIN_MODULES: &[&str] = &[
  "assert",
  "async_hooks",
  "buffer",
  "child_process",
  "cluster",
  "console",
  "constants",
  "crypto",
  "dgram",
  "diagnostics_channel",
  "dns",
  "domain",
  "events",
  "fs",
  "http",
  "http2",
  "https",
  "inspector",
  "module",
  "net",
  "os",
  "path",
  "perf_hooks",
  "process",
  "punycode",
  "querystring",
  "readline",
  "repl",
  "stream",
  "string_decoder",
  "sys",
  "timers",
  "tls",
  "trace_events",
  "tty",
  "url",
  "util",
  "v8",
  "vm",
  "wasi",
  "worker_threads",
  "zlib",
];

/// Node.js built-in subpath modules.
const NODE_BUILTIN_SUBPATHS: &[&str] = &[
  "assert/strict",
  "dns/promises",
  "fs/promises",
  "path/posix",
  "path/win32",
  "readline/promises",
  "stream/consumers",
  "stream/promises",
  "stream/web",
  "timers/promises",
  "util/types",
];

/// Generate external patterns for all Node.js built-in modules.
///
/// Returns patterns like `"node:fs"`, `"node:fs/*"`, `"fs"`, `"fs/*"`.
pub fn node_builtin_external_patterns() -> Vec<String> {
  let mut patterns = Vec::new();
  for module in NODE_BUILTIN_MODULES {
    patterns.push(format!("node:{module}"));
    patterns.push(format!("node:{module}/*"));
    patterns.push((*module).to_string());
    patterns.push(format!("{module}/*"));
  }
  for subpath in NODE_BUILTIN_SUBPATHS {
    patterns.push(format!("node:{subpath}"));
    patterns.push((*subpath).to_string());
  }
  patterns
}

/// Normalize a node specifier by stripping the `node:` prefix if present.
pub fn normalize_node_specifier(specifier: &str) -> String {
  specifier
    .strip_prefix("node:")
    .unwrap_or(specifier)
    .to_string()
}

/// Check if a specifier matches any of the external patterns.
///
/// Patterns support trailing `*` as a wildcard for prefix matching.
pub fn is_external(specifier: &str, patterns: &[String]) -> bool {
  patterns
    .iter()
    .any(|pattern| matches_pattern(specifier, pattern))
}

fn matches_pattern(specifier: &str, pattern: &str) -> bool {
  if let Some(prefix) = pattern.strip_suffix('*') {
    specifier.starts_with(prefix)
  } else {
    specifier == pattern
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_external_exact() {
    let patterns = vec!["node:fs".to_string()];
    assert!(is_external("node:fs", &patterns));
    assert!(!is_external("node:path", &patterns));
  }

  #[test]
  fn test_is_external_wildcard() {
    let patterns = vec!["node:fs/*".to_string()];
    assert!(is_external("node:fs/promises", &patterns));
    assert!(!is_external("node:fs", &patterns));
  }

  #[test]
  fn test_node_builtins() {
    let patterns = node_builtin_external_patterns();
    assert!(is_external("node:fs", &patterns));
    assert!(is_external("node:fs/promises", &patterns));
    assert!(is_external("fs", &patterns));
    assert!(!is_external("react", &patterns));
  }
}
