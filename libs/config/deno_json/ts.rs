// Copyright 2018-2025 the Deno authors. MIT license.

use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use serde_json::Value;

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
