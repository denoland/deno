// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt;

use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use serde_json::Value;
use url::Url;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawJsxCompilerOptions {
  pub jsx: Option<String>,
  pub jsx_import_source: Option<String>,
  pub jsx_import_source_types: Option<String>,
}

/// The transpile options that are significant out of a user provided tsconfig
/// file, that we want to deserialize out of the final config for a transpile.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitConfigOptions {
  pub check_js: bool,
  pub experimental_decorators: bool,
  pub emit_decorator_metadata: bool,
  pub imports_not_used_as_values: String,
  pub inline_source_map: bool,
  pub inline_sources: bool,
  pub source_map: bool,
  pub jsx: String,
  pub jsx_factory: String,
  pub jsx_fragment_factory: String,
  pub jsx_import_source: Option<String>,
  pub jsx_precompile_skip_elements: Option<Vec<String>>,
}

/// A structure that represents a set of options that were ignored and the
/// path those options came from.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IgnoredCompilerOptions {
  pub items: Vec<String>,
  pub maybe_specifier: Option<Url>,
}

impl fmt::Display for IgnoredCompilerOptions {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut codes = self.items.clone();
    codes.sort_unstable();
    if let Some(specifier) = &self.maybe_specifier {
      write!(f, "Unsupported compiler options in \"{}\".\n  The following options were ignored:\n    {}", specifier, codes.join(", "))
    } else {
      write!(f, "Unsupported compiler options provided.\n  The following options were ignored:\n    {}", codes.join(", "))
    }
  }
}

impl Serialize for IgnoredCompilerOptions {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    Serialize::serialize(&self.items, serializer)
  }
}

/// A set of all the compiler options that should be allowed;
static ALLOWED_COMPILER_OPTIONS: phf::Set<&'static str> = phf::phf_set! {
  "allowUnreachableCode",
  "allowUnusedLabels",
  "checkJs",
  "erasableSyntaxOnly",
  "emitDecoratorMetadata",
  "exactOptionalPropertyTypes",
  "experimentalDecorators",
  "isolatedDeclarations",
  "jsx",
  "jsxFactory",
  "jsxFragmentFactory",
  "jsxImportSource",
  "jsxPrecompileSkipElements",
  "lib",
  "noErrorTruncation",
  "noFallthroughCasesInSwitch",
  "noImplicitAny",
  "noImplicitOverride",
  "noImplicitReturns",
  "noImplicitThis",
  "noPropertyAccessFromIndexSignature",
  "noUncheckedIndexedAccess",
  "noUnusedLocals",
  "noUnusedParameters",
  "rootDirs",
  "strict",
  "strictBindCallApply",
  "strictBuiltinIteratorReturn",
  "strictFunctionTypes",
  "strictNullChecks",
  "strictPropertyInitialization",
  "types",
  "useUnknownInCatchVariables",
  "verbatimModuleSyntax",
};

#[derive(Debug, Default, Clone)]
pub struct ParsedCompilerOptions {
  pub options: serde_json::Map<String, serde_json::Value>,
  pub maybe_ignored: Option<IgnoredCompilerOptions>,
}

pub fn parse_compiler_options(
  compiler_options: serde_json::Map<String, Value>,
  maybe_specifier: Option<&Url>,
) -> ParsedCompilerOptions {
  let mut allowed: serde_json::Map<String, Value> =
    serde_json::Map::with_capacity(compiler_options.len());
  let mut ignored: Vec<String> = Vec::new(); // don't pre-allocate because it's rare

  for (key, value) in compiler_options {
    // We don't pass "types" entries to typescript via the compiler
    // options and instead provide those to tsc as "roots". This is
    // because our "types" behavior is at odds with how TypeScript's
    // "types" works.
    // We also don't pass "jsxImportSourceTypes" to TypeScript as it doesn't
    // know about this option. It will still take this option into account
    // because the graph resolves the JSX import source to the types for TSC.
    if key != "types" && key != "jsxImportSourceTypes" {
      if ALLOWED_COMPILER_OPTIONS.contains(key.as_str()) {
        allowed.insert(key, value.to_owned());
      } else {
        ignored.push(key);
      }
    }
  }
  let maybe_ignored = if !ignored.is_empty() {
    Some(IgnoredCompilerOptions {
      items: ignored,
      maybe_specifier: maybe_specifier.cloned(),
    })
  } else {
    None
  };

  ParsedCompilerOptions {
    options: allowed,
    maybe_ignored,
  }
}

/// A structure for managing the configuration of TypeScript
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompilerOptions(pub Value);

impl Default for CompilerOptions {
  fn default() -> Self {
    Self(serde_json::Value::Object(Default::default()))
  }
}

impl CompilerOptions {
  /// Create a new `CompilerOptions` with the base being the `value` supplied.
  pub fn new(value: Value) -> Self {
    CompilerOptions(value)
  }

  pub fn merge_mut(&mut self, value: CompilerOptions) {
    json_merge(&mut self.0, value.0);
  }

  /// Merge a serde_json value into the configuration.
  pub fn merge_object_mut(
    &mut self,
    value: serde_json::Map<String, serde_json::Value>,
  ) {
    json_merge(&mut self.0, serde_json::Value::Object(value));
  }
}

impl Serialize for CompilerOptions {
  /// Serializes inner hash map which is ordered by the key
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    Serialize::serialize(&self.0, serializer)
  }
}

/// A function that works like JavaScript's `Object.assign()`.
fn json_merge(a: &mut Value, b: Value) {
  match (a, b) {
    (&mut Value::Object(ref mut a), Value::Object(b)) => {
      for (k, v) in b {
        json_merge(a.entry(k).or_insert(Value::Null), v);
      }
    }
    (a, b) => {
      *a = b;
    }
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn test_json_merge() {
    let mut value_a = json!({
      "a": true,
      "b": "c"
    });
    let value_b = json!({
      "b": "d",
      "e": false,
    });
    json_merge(&mut value_a, value_b);
    assert_eq!(
      value_a,
      json!({
        "a": true,
        "b": "d",
        "e": false,
      })
    );
  }
}
