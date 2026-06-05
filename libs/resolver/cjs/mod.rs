// Copyright 2018-2026 the Deno authors. MIT license.

use deno_maybe_sync::MaybeDashMap;
use deno_media_type::MediaType;
use node_resolver::InNpmPackageChecker;
use node_resolver::PackageJsonResolverRc;
use node_resolver::ResolutionMode;
use node_resolver::errors::PackageJsonLoadError;
use serde_json::Value;
use sys_traits::FsMetadata;
use sys_traits::FsRead;
use url::Url;

pub mod analyzer;

#[allow(clippy::disallowed_types, reason = "definition")]
pub type CjsTrackerRc<TInNpmPackageChecker, TSys> =
  deno_maybe_sync::MaybeArc<CjsTracker<TInNpmPackageChecker, TSys>>;

/// Keeps track of what module specifiers were resolved as CJS.
///
/// Modules that are `.js`, `.ts`, `.jsx`, and `tsx` are only known to
/// be CJS or ESM after they're loaded based on their contents. So these
/// files will be "maybe CJS" until they're loaded.
#[derive(Debug)]
pub struct CjsTracker<
  TInNpmPackageChecker: InNpmPackageChecker,
  TSys: FsRead + FsMetadata,
> {
  is_cjs_resolver: IsCjsResolver<TInNpmPackageChecker, TSys>,
  known: MaybeDashMap<Url, ResolutionMode>,
  require_modules: Vec<Url>,
}

impl<TInNpmPackageChecker: InNpmPackageChecker, TSys: FsRead + FsMetadata>
  CjsTracker<TInNpmPackageChecker, TSys>
{
  pub fn new(
    in_npm_pkg_checker: TInNpmPackageChecker,
    pkg_json_resolver: PackageJsonResolverRc<TSys>,
    mode: IsCjsResolutionMode,
    require_modules: Vec<Url>,
  ) -> Self {
    Self {
      is_cjs_resolver: IsCjsResolver::new(
        in_npm_pkg_checker,
        pkg_json_resolver,
        mode,
      ),
      known: Default::default(),
      require_modules,
    }
  }

  /// Checks whether the file might be treated as CJS, but it's not for sure
  /// yet because the source hasn't been loaded to see whether it contains
  /// imports or exports.
  pub fn is_maybe_cjs(
    &self,
    specifier: &Url,
    media_type: MediaType,
  ) -> Result<bool, PackageJsonLoadError> {
    self.treat_as_cjs_with_is_script(specifier, media_type, None)
  }

  /// Checks whether a file loaded via `require()` should be compiled as
  /// CommonJS before falling back to ESM syntax detection.
  pub fn is_maybe_cjs_from_require(
    &self,
    specifier: &Url,
    media_type: MediaType,
  ) -> Result<bool, PackageJsonLoadError> {
    self
      .is_cjs_resolver
      .check_for_require(specifier, media_type)
      .map(|mode| mode == ResolutionMode::Require)
  }

  /// Mark a file as being known CJS or ESM.
  pub fn set_is_known_script(&self, specifier: &Url, is_script: bool) {
    let new_value = if is_script {
      ResolutionMode::Require
    } else {
      ResolutionMode::Import
    };
    // block to really ensure dashmap is not borrowed while trying to insert
    {
      if let Some(value) = self.known.get(specifier) {
        // you shouldn't be insert a value in here that's
        // already known and is a different value than what
        // was previously determined
        debug_assert_eq!(*value, new_value);
        return;
      }
    }
    self.known.insert(specifier.clone(), new_value);
  }

  /// Gets whether the file is CJS. If true, this is for sure
  /// cjs because `is_script` is provided.
  ///
  /// `is_script` should be `true` when the contents of the file at the
  /// provided specifier are known to be a script and not an ES module.
  pub fn is_cjs_with_known_is_script(
    &self,
    specifier: &Url,
    media_type: MediaType,
    is_script: bool,
  ) -> Result<bool, PackageJsonLoadError> {
    self.treat_as_cjs_with_is_script(specifier, media_type, Some(is_script))
  }

  fn treat_as_cjs_with_is_script(
    &self,
    specifier: &Url,
    media_type: MediaType,
    is_script: Option<bool>,
  ) -> Result<bool, PackageJsonLoadError> {
    let kind = match self
      .get_known_mode_with_is_script(specifier, media_type, is_script)
    {
      Some(kind) => kind,
      None => self.is_cjs_resolver.check_based_on_pkg_json(specifier)?,
    };
    Ok(kind == ResolutionMode::Require)
  }

  /// Gets the referrer for the specified module specifier.
  ///
  /// Generally the referrer should already be tracked by calling
  /// `is_cjs_with_known_is_script` before calling this method.
  pub fn get_referrer_kind(&self, specifier: &Url) -> ResolutionMode {
    if specifier.scheme() != "file" {
      return ResolutionMode::Import;
    }
    self
      .get_known_mode(specifier, MediaType::from_specifier(specifier))
      .unwrap_or(ResolutionMode::Import)
  }

  fn get_known_mode(
    &self,
    specifier: &Url,
    media_type: MediaType,
  ) -> Option<ResolutionMode> {
    self.get_known_mode_with_is_script(specifier, media_type, None)
  }

  fn get_known_mode_with_is_script(
    &self,
    specifier: &Url,
    media_type: MediaType,
    is_script: Option<bool>,
  ) -> Option<ResolutionMode> {
    let is_from_require = self.require_modules.contains(specifier);
    self.is_cjs_resolver.get_known_mode_with_is_script(
      specifier,
      media_type,
      is_script,
      is_from_require,
      &self.known,
    )
  }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum IsCjsResolutionMode {
  /// Requires an explicit `"type": "commonjs"` in the package.json.
  ExplicitTypeCommonJs,
  /// Implicitly uses `"type": "commonjs"` if no `"type"` is specified.
  ImplicitTypeCommonJs,
  /// Does not respect `"type": "commonjs"` and always treats ambiguous files as ESM.
  #[default]
  Disabled,
}

/// Resolves whether a module is CJS or ESM.
#[derive(Debug)]
pub struct IsCjsResolver<
  TInNpmPackageChecker: InNpmPackageChecker,
  TSys: FsRead + FsMetadata,
> {
  in_npm_pkg_checker: TInNpmPackageChecker,
  pkg_json_resolver: PackageJsonResolverRc<TSys>,
  mode: IsCjsResolutionMode,
}

impl<TInNpmPackageChecker: InNpmPackageChecker, TSys: FsRead + FsMetadata>
  IsCjsResolver<TInNpmPackageChecker, TSys>
{
  pub fn new(
    in_npm_pkg_checker: TInNpmPackageChecker,
    pkg_json_resolver: PackageJsonResolverRc<TSys>,
    mode: IsCjsResolutionMode,
  ) -> Self {
    Self {
      in_npm_pkg_checker,
      pkg_json_resolver,
      mode,
    }
  }

  /// Gets the resolution mode for a module in the LSP.
  pub fn get_lsp_resolution_mode(
    &self,
    specifier: &Url,
    is_script: Option<bool>,
  ) -> ResolutionMode {
    if specifier.scheme() != "file" {
      return ResolutionMode::Import;
    }
    match MediaType::from_specifier(specifier) {
      MediaType::Mts | MediaType::Mjs | MediaType::Dmts => ResolutionMode::Import,
      MediaType::Cjs | MediaType::Cts | MediaType::Dcts => ResolutionMode::Require,
      MediaType::Dts => {
        // Inside npm packages, the .d.ts content can override the package.json
        // signal: if it uses TypeScript-only CJS syntax (`export =` etc.) it
        // is CJS; if it uses ESM-style syntax it may be ESM even when the
        // package.json has no `"type": "module"` (issue #28071). Outside of
        // npm packages, keep the previous behavior of trusting the
        // package.json so local `.d.ts` files (e.g. shimmed via
        // `@deno-types`) are not affected.
        if self.in_npm_pkg_checker.in_npm_package(specifier)
          && is_script == Some(true)
        {
          return ResolutionMode::Require;
        }
        self
          .check_dts_based_on_pkg_json(specifier)
          .unwrap_or(ResolutionMode::Import)
      }
      MediaType::Wasm |
      MediaType::Json => ResolutionMode::Import,
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::TypeScript
      | MediaType::Tsx
      // treat these as unknown
      | MediaType::Css
      | MediaType::Html
      | MediaType::Jsonc
      | MediaType::Json5
      | MediaType::Markdown
      | MediaType::SourceMap
      | MediaType::Sql
      | MediaType::Unknown => {
        match is_script {
          Some(true) => self.check_based_on_pkg_json(specifier).unwrap_or(ResolutionMode::Import),
          Some(false) | None => ResolutionMode::Import,
        }
      }
    }
  }

  fn get_known_mode_with_is_script(
    &self,
    specifier: &Url,
    media_type: MediaType,
    is_script: Option<bool>,
    is_from_require: bool,
    known_cache: &MaybeDashMap<Url, ResolutionMode>,
  ) -> Option<ResolutionMode> {
    if specifier.scheme() != "file" {
      return Some(ResolutionMode::Import);
    }

    match media_type {
      MediaType::Mts | MediaType::Mjs | MediaType::Dmts => Some(ResolutionMode::Import),
      MediaType::Cjs | MediaType::Cts | MediaType::Dcts => Some(ResolutionMode::Require),
      MediaType::Dts => {
        // Inside npm packages, the .d.ts content can override the package.json
        // signal — see `get_lsp_resolution_mode`. Outside npm packages, keep
        // the previous behavior of trusting the package.json so local .d.ts
        // files (e.g. shimmed via `@deno-types`) are not affected.
        if self.in_npm_pkg_checker.in_npm_package(specifier)
          && is_script == Some(true)
        {
          known_cache.insert(specifier.clone(), ResolutionMode::Require);
          return Some(ResolutionMode::Require);
        }
        if let Some(value) = known_cache.get(specifier).map(|v| *v) {
          Some(value)
        } else {
          let value = self.check_dts_based_on_pkg_json(specifier).ok();
          if let Some(value) = value {
            known_cache.insert(specifier.clone(), value);
          }
          Some(value.unwrap_or(ResolutionMode::Import))
        }
      }
      MediaType::Wasm |
      MediaType::Json => Some(ResolutionMode::Import),
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::TypeScript
      | MediaType::Tsx
      // treat these as unknown
      | MediaType::Css
      | MediaType::Html
      | MediaType::Jsonc
      | MediaType::Json5
      | MediaType::Markdown
      | MediaType::SourceMap
      | MediaType::Sql
      | MediaType::Unknown => {
        if is_from_require {
          return Some(ResolutionMode::Require);
        }

        if let Some(value) = known_cache.get(specifier).map(|v| *v) {
          if value == ResolutionMode::Require && is_script == Some(false) {
            // we now know this is actually esm
            known_cache.insert(specifier.clone(), ResolutionMode::Import);
            Some(ResolutionMode::Import)
          } else {
            Some(value)
          }
        } else if is_script == Some(false) {
          // we know this is esm
          known_cache.insert(specifier.clone(), ResolutionMode::Import);
          Some(ResolutionMode::Import)
        } else {
          None
        }
      }
    }
  }

  /// Like [`check_based_on_pkg_json`] but for `.d.ts` files in npm packages.
  ///
  /// In addition to the standard `"type"` field check, this also treats the
  /// `.d.ts` as ESM when the package opts into ESM via an `"import"`
  /// condition in its `exports` map. Many packages (e.g. `@rollup/plugin-replace`)
  /// publish a single `.d.ts` shared between the `import` and `require`
  /// conditions and rely on consumers using ESM syntax. Treating those `.d.ts`
  /// files as CJS causes `import x from "pkg"` to be typed as the namespace
  /// rather than the default export. See issue #28071.
  fn check_dts_based_on_pkg_json(
    &self,
    specifier: &Url,
  ) -> Result<ResolutionMode, PackageJsonLoadError> {
    if self.in_npm_pkg_checker.in_npm_package(specifier) {
      let Ok(path) = deno_path_util::url_to_file_path(specifier) else {
        return Ok(ResolutionMode::Require);
      };
      if let Some(pkg_json) =
        self.pkg_json_resolver.get_closest_package_json(&path)?
      {
        if pkg_json.typ == "module" {
          return Ok(ResolutionMode::Import);
        }
        if let Some(exports) = pkg_json.exports.as_ref()
          && exports_has_import_condition(exports)
        {
          return Ok(ResolutionMode::Import);
        }
      }
    }
    self.check_based_on_pkg_json(specifier)
  }

  fn check_based_on_pkg_json(
    &self,
    specifier: &Url,
  ) -> Result<ResolutionMode, PackageJsonLoadError> {
    if self.in_npm_pkg_checker.in_npm_package(specifier) {
      let Ok(path) = deno_path_util::url_to_file_path(specifier) else {
        return Ok(ResolutionMode::Require);
      };
      if let Some(pkg_json) =
        self.pkg_json_resolver.get_closest_package_json(&path)?
      {
        let is_file_location_cjs = pkg_json.typ != "module";
        Ok(if is_file_location_cjs || path.extension().is_none() {
          ResolutionMode::Require
        } else {
          ResolutionMode::Import
        })
      } else {
        Ok(ResolutionMode::Require)
      }
    } else if self.mode != IsCjsResolutionMode::Disabled {
      let Ok(path) = deno_path_util::url_to_file_path(specifier) else {
        return Ok(ResolutionMode::Import);
      };
      if let Some(pkg_json) =
        self.pkg_json_resolver.get_closest_package_json(&path)?
      {
        let is_cjs_type = pkg_json.typ == "commonjs"
          || self.mode == IsCjsResolutionMode::ImplicitTypeCommonJs
            && pkg_json.typ == "none";
        Ok(if is_cjs_type {
          ResolutionMode::Require
        } else {
          ResolutionMode::Import
        })
      } else if self.mode == IsCjsResolutionMode::ImplicitTypeCommonJs {
        Ok(ResolutionMode::Require)
      } else {
        Ok(ResolutionMode::Import)
      }
    } else {
      Ok(ResolutionMode::Import)
    }
  }

  fn check_for_require(
    &self,
    specifier: &Url,
    media_type: MediaType,
  ) -> Result<ResolutionMode, PackageJsonLoadError> {
    if specifier.scheme() != "file" {
      return Ok(ResolutionMode::Import);
    }

    match media_type {
      MediaType::Mts | MediaType::Mjs | MediaType::Dmts => {
        Ok(ResolutionMode::Import)
      }
      MediaType::Cjs | MediaType::Cts | MediaType::Dcts => {
        Ok(ResolutionMode::Require)
      }
      MediaType::Wasm | MediaType::Json => Ok(ResolutionMode::Import),
      MediaType::Dts
      | MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::TypeScript
      | MediaType::Tsx
      | MediaType::Css
      | MediaType::Html
      | MediaType::Jsonc
      | MediaType::Json5
      | MediaType::Markdown
      | MediaType::SourceMap
      | MediaType::Sql
      | MediaType::Unknown => {
        let Ok(path) = deno_path_util::url_to_file_path(specifier) else {
          return Ok(ResolutionMode::Import);
        };
        let Some(pkg_json) =
          self.pkg_json_resolver.get_closest_package_json(&path)?
        else {
          return Ok(ResolutionMode::Require);
        };
        Ok(if pkg_json.typ == "module" && path.extension().is_some() {
          ResolutionMode::Import
        } else {
          ResolutionMode::Require
        })
      }
    }
  }
}

/// Returns true if the given package `exports` value contains an `"import"`
/// condition anywhere in the (possibly nested) condition map. This is used as
/// a hint that the package supports ESM at runtime, in which case its `.d.ts`
/// files should be treated as ESM by the type checker.
///
/// This signal is package-global: the `.d.ts` being classified might be reached
/// through a CJS-only subpath while a different subpath carries the `import`
/// condition. The precise fix would key off the specific export entry that
/// resolved to this file, but that information isn't available at this layer,
/// so a `.d.ts` belonging to a CJS-only subpath of a mixed CJS/ESM package may
/// still be flipped to ESM. The shared-single-`.d.ts` case (issue #28071,
/// e.g. `@rollup/plugin-replace`) is the motivating shape; the mixed case is
/// covered by a spec test that pins the chosen behavior.
fn exports_has_import_condition(
  exports: &serde_json::Map<String, Value>,
) -> bool {
  fn value_has_import_condition(value: &Value) -> bool {
    match value {
      Value::Object(map) => map_has_import_condition(map),
      Value::Array(arr) => arr.iter().any(value_has_import_condition),
      _ => false,
    }
  }
  fn map_has_import_condition(map: &serde_json::Map<String, Value>) -> bool {
    for (key, v) in map {
      if key == "import" {
        // ignore null targets, which the exports algorithm treats as
        // "no resolution".
        if !v.is_null() {
          return true;
        }
      } else if value_has_import_condition(v) {
        return true;
      }
    }
    false
  }
  map_has_import_condition(exports)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(json: &str) -> serde_json::Map<String, Value> {
    serde_json::from_str(json).unwrap()
  }

  #[test]
  fn exports_with_import_condition() {
    assert!(exports_has_import_condition(&parse(
      r#"{"types": "./types/index.d.ts", "import": "./dist/es/index.js", "default": "./dist/cjs/index.js"}"#
    )));
    assert!(exports_has_import_condition(&parse(
      r#"{".": {"import": "./dist/es/index.js", "require": "./dist/cjs/index.js"}}"#
    )));
    assert!(exports_has_import_condition(&parse(
      r#"{".": {"node": {"import": "./dist/es/index.js"}}}"#
    )));
    assert!(!exports_has_import_condition(&parse(
      r#"{".": {"require": "./dist/cjs/index.js", "default": "./dist/cjs/index.js"}}"#
    )));
    assert!(!exports_has_import_condition(&parse(
      r#"{".": "./dist/cjs/index.js"}"#
    )));
    assert!(!exports_has_import_condition(&parse(
      r#"{".": {"import": null, "require": "./dist/cjs/index.js"}}"#
    )));
  }
}
