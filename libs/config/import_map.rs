// Copyright 2018-2026 the Deno authors. MIT license.

use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use import_map::ImportMap;

pub fn import_map_deps(
  import_map: &ImportMap,
) -> impl Iterator<Item = JsrDepPackageReq> {
  fn from_map(
    specifier_map: &import_map::SpecifierMap,
  ) -> impl Iterator<Item = &str> {
    specifier_map.entries().flat_map(|e| e.raw_value)
  }

  from_map(import_map.imports())
    .chain(
      import_map
        .scopes()
        .flat_map(|scope| from_map(scope.imports)),
    )
    .filter_map(value_to_dep_req)
}

/// Attempts to resolve any `npm:` and `jsr:` dependencies
/// in the import map's imports and scopes.
pub fn import_map_deps_from_value(
  import_map: &serde_json::Value,
) -> impl Iterator<Item = JsrDepPackageReq> {
  imports_values(import_map.get("imports"))
    .chain(scope_values(import_map.get("scopes")))
    .filter_map(value_to_dep_req)
}

pub(crate) fn imports_values(
  value: Option<&serde_json::Value>,
) -> impl Iterator<Item = &str> {
  imports_entries(value).map(|(_, v)| v)
}

pub(crate) fn scope_values(
  value: Option<&serde_json::Value>,
) -> impl Iterator<Item = &str> {
  scope_entries(value).map(|(_, v)| v)
}

pub(crate) fn imports_entries(
  value: Option<&serde_json::Value>,
) -> impl Iterator<Item = (&str, &str)> {
  value
    .and_then(|v| v.as_object())
    .map(|obj| {
      obj.iter().filter_map(|(k, v)| match v {
        serde_json::Value::String(value) => Some((k.as_str(), value.as_ref())),
        _ => None,
      })
    })
    .into_iter()
    .flatten()
}

pub(crate) fn scope_entries(
  value: Option<&serde_json::Value>,
) -> impl Iterator<Item = (&str, &str)> {
  value
    .and_then(|v| v.as_object())
    .map(|obj| obj.values().flat_map(|v| imports_entries(Some(v))))
    .into_iter()
    .flatten()
}

pub(crate) fn value_to_dep_req(value: &str) -> Option<JsrDepPackageReq> {
  match JsrPackageReqReference::from_str(value) {
    Ok(req_ref) => Some(JsrDepPackageReq::jsr(req_ref.into_inner().req)),
    _ => match NpmPackageReqReference::from_str(value) {
      Ok(req_ref) => Some(JsrDepPackageReq::npm(req_ref.into_inner().req)),
      _ => None,
    },
  }
}

/// Rewrites a `jsr:` import specifier to the equivalent `npm:` specifier that
/// targets JSR's npm compatibility registry. JSR mirrors every package onto
/// npm under the `@jsr/` scope, joining the original scope and name with `__`
/// (e.g. `jsr:@david/dax@^0.48` -> `npm:@jsr/david__dax@^0.48`). This lets JSR
/// dependencies be installed into `node_modules` and resolved by Node-style
/// resolution like any other npm package.
///
/// Returns `None` when `value` is not a parseable `jsr:` specifier.
pub fn jsr_specifier_to_npm(value: &str) -> Option<String> {
  jsr_specifier_to_npm_parts(value).map(|(rewritten, _has_subpath)| rewritten)
}

/// Like [`jsr_specifier_to_npm`] but also reports whether the original
/// specifier carried a subpath. Parsing happens exactly once so callers that
/// need both the rewritten string and the subpath flag stay consistent (a
/// trailing slash is stripped before parsing in either case).
fn jsr_specifier_to_npm_parts(value: &str) -> Option<(String, bool)> {
  // A trailing slash marks an import-map prefix mapping (e.g. the expanded
  // `@david/dax/` -> `jsr:@david/dax@^0.42/`). Strip it for parsing and
  // re-append it to the npm target so subpath imports keep resolving.
  let (core, trailing_slash) = match value.strip_suffix('/') {
    Some(core) => (core, true),
    None => (value, false),
  };
  let req_ref = JsrPackageReqReference::from_str(core).ok()?;
  let req = req_ref.req();
  let (scope, name) = req.name.strip_prefix('@')?.split_once('/')?;
  let mut out = format!("npm:@jsr/{}__{}", scope, name);
  let version_text = req.version_req.version_text();
  if !version_text.is_empty() && version_text != "*" {
    out.push('@');
    out.push_str(version_text);
  }
  let has_subpath = req_ref.sub_path().is_some();
  if let Some(sub_path) = req_ref.sub_path() {
    out.push('/');
    out.push_str(sub_path);
  }
  if trailing_slash {
    out.push('/');
  }
  Some((out, has_subpath))
}

/// In-place rewrite of every `jsr:` specifier within an import map
/// `serde_json::Value` (both its `imports` and `scopes`) to the npm-compat
/// form via [`jsr_specifier_to_npm`]. Non-`jsr:` values are left untouched.
///
/// This is used when a `node_modules` directory is in play so that JSR
/// dependencies are installed and resolved through the npm machinery.
pub fn rewrite_jsr_imports_to_npm(value: &mut serde_json::Value) {
  fn rewrite_map(map: &mut serde_json::Map<String, serde_json::Value>) {
    // Entries to add after iterating: bare package mappings get a matching
    // trailing-slash prefix entry so subpath imports (e.g. `@david/dax/x.ts`
    // or package assets) keep resolving once mapped onto npm. The import map
    // standard requires both `target` and the trailing-slash prefix `target/`.
    let mut prefix_additions: Vec<(String, String)> = Vec::new();
    for (key, v) in map.iter_mut() {
      if let serde_json::Value::String(s) = v
        && let Some((rewritten, has_subpath)) = jsr_specifier_to_npm_parts(s)
      {
        // Only bare package mappings (no subpath in the original value) get a
        // trailing-slash prefix entry; a value with a subpath already targets a
        // specific file.
        if !key.ends_with('/') && !has_subpath {
          prefix_additions.push((format!("{key}/"), format!("{rewritten}/")));
        }
        *s = rewritten;
      }
    }
    for (key, target) in prefix_additions {
      map
        .entry(key)
        .or_insert_with(|| serde_json::Value::String(target));
    }
  }

  let Some(obj) = value.as_object_mut() else {
    return;
  };
  if let Some(serde_json::Value::Object(imports)) = obj.get_mut("imports") {
    rewrite_map(imports);
  }
  if let Some(serde_json::Value::Object(scopes)) = obj.get_mut("scopes") {
    for scope in scopes.values_mut() {
      if let serde_json::Value::Object(scope_map) = scope {
        rewrite_map(scope_map);
      }
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn jsr_specifier_to_npm_rewrites() {
    assert_eq!(
      jsr_specifier_to_npm("jsr:@david/dax@^0.48").as_deref(),
      Some("npm:@jsr/david__dax@^0.48")
    );
    // no version
    assert_eq!(
      jsr_specifier_to_npm("jsr:@std/bytes").as_deref(),
      Some("npm:@jsr/std__bytes")
    );
    // subpath
    assert_eq!(
      jsr_specifier_to_npm("jsr:@std/encoding@^1/hex").as_deref(),
      Some("npm:@jsr/std__encoding@^1/hex")
    );
    // trailing-slash prefix mapping (from import map expansion)
    assert_eq!(
      jsr_specifier_to_npm("jsr:@david/dax@^0.48/").as_deref(),
      Some("npm:@jsr/david__dax@^0.48/")
    );
    // not a jsr specifier
    assert_eq!(jsr_specifier_to_npm("npm:chalk@5"), None);
    assert_eq!(jsr_specifier_to_npm("./local.ts"), None);
  }

  #[test]
  fn rewrite_jsr_imports_to_npm_imports_and_scopes() {
    let mut value = serde_json::json!({
      "imports": {
        "@david/dax": "jsr:@david/dax@^0.48",
        "chalk": "npm:chalk@5"
      },
      "scopes": {
        "./sub/": { "@std/bytes": "jsr:@std/bytes@^1" }
      }
    });
    rewrite_jsr_imports_to_npm(&mut value);
    assert_eq!(
      value["imports"]["@david/dax"],
      serde_json::json!("npm:@jsr/david__dax@^0.48")
    );
    // adds a trailing-slash prefix entry for subpath imports
    assert_eq!(
      value["imports"]["@david/dax/"],
      serde_json::json!("npm:@jsr/david__dax@^0.48/")
    );
    // leaves npm specifiers alone
    assert_eq!(value["imports"]["chalk"], serde_json::json!("npm:chalk@5"));
    assert_eq!(
      value["scopes"]["./sub/"]["@std/bytes"],
      serde_json::json!("npm:@jsr/std__bytes@^1")
    );
  }
}
