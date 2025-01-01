// Copyright 2018-2025 the Deno authors. MIT license.

use deno_media_type::MediaType;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::InNpmPackageCheckerRc;
use node_resolver::PackageJsonResolverRc;
use node_resolver::ResolutionMode;
use sys_traits::FsRead;
use url::Url;

use crate::sync::MaybeDashMap;

/// Keeps track of what module specifiers were resolved as CJS.
///
/// Modules that are `.js`, `.ts`, `.jsx`, and `tsx` are only known to
/// be CJS or ESM after they're loaded based on their contents. So these
/// files will be "maybe CJS" until they're loaded.
#[derive(Debug)]
pub struct CjsTracker<TSys: FsRead> {
  is_cjs_resolver: IsCjsResolver<TSys>,
  known: MaybeDashMap<Url, ResolutionMode>,
}

impl<TSys: FsRead> CjsTracker<TSys> {
  pub fn new(
    in_npm_pkg_checker: InNpmPackageCheckerRc,
    pkg_json_resolver: PackageJsonResolverRc<TSys>,
    mode: IsCjsResolutionMode,
  ) -> Self {
    Self {
      is_cjs_resolver: IsCjsResolver::new(
        in_npm_pkg_checker,
        pkg_json_resolver,
        mode,
      ),
      known: Default::default(),
    }
  }

  /// Checks whether the file might be treated as CJS, but it's not for sure
  /// yet because the source hasn't been loaded to see whether it contains
  /// imports or exports.
  pub fn is_maybe_cjs(
    &self,
    specifier: &Url,
    media_type: MediaType,
  ) -> Result<bool, ClosestPkgJsonError> {
    self.treat_as_cjs_with_is_script(specifier, media_type, None)
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
  ) -> Result<bool, ClosestPkgJsonError> {
    self.treat_as_cjs_with_is_script(specifier, media_type, Some(is_script))
  }

  fn treat_as_cjs_with_is_script(
    &self,
    specifier: &Url,
    media_type: MediaType,
    is_script: Option<bool>,
  ) -> Result<bool, ClosestPkgJsonError> {
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
    self.is_cjs_resolver.get_known_mode_with_is_script(
      specifier,
      media_type,
      is_script,
      &self.known,
    )
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsCjsResolutionMode {
  /// Requires an explicit `"type": "commonjs"` in the package.json.
  ExplicitTypeCommonJs,
  /// Implicitly uses `"type": "commonjs"` if no `"type"` is specified.
  ImplicitTypeCommonJs,
  /// Does not respect `"type": "commonjs"` and always treats ambiguous files as ESM.
  Disabled,
}

/// Resolves whether a module is CJS or ESM.
#[derive(Debug)]
pub struct IsCjsResolver<TSys: FsRead> {
  in_npm_pkg_checker: InNpmPackageCheckerRc,
  pkg_json_resolver: PackageJsonResolverRc<TSys>,
  mode: IsCjsResolutionMode,
}

impl<TSys: FsRead> IsCjsResolver<TSys> {
  pub fn new(
    in_npm_pkg_checker: InNpmPackageCheckerRc,
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
        // dts files are always determined based on the package.json because
        // they contain imports/exports even when considered CJS
        self.check_based_on_pkg_json(specifier).unwrap_or(ResolutionMode::Import)
      }
      MediaType::Wasm |
      MediaType::Json => ResolutionMode::Import,
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::TypeScript
      | MediaType::Tsx
      // treat these as unknown
      | MediaType::Css
      | MediaType::SourceMap
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
    known_cache: &MaybeDashMap<Url, ResolutionMode>,
  ) -> Option<ResolutionMode> {
    if specifier.scheme() != "file" {
      return Some(ResolutionMode::Import);
    }

    match media_type {
      MediaType::Mts | MediaType::Mjs | MediaType::Dmts => Some(ResolutionMode::Import),
      MediaType::Cjs | MediaType::Cts | MediaType::Dcts => Some(ResolutionMode::Require),
      MediaType::Dts => {
        // dts files are always determined based on the package.json because
        // they contain imports/exports even when considered CJS
        if let Some(value) = known_cache.get(specifier).map(|v| *v) {
          Some(value)
        } else {
          let value = self.check_based_on_pkg_json(specifier).ok();
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
      | MediaType::SourceMap
      | MediaType::Unknown => {
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

  fn check_based_on_pkg_json(
    &self,
    specifier: &Url,
  ) -> Result<ResolutionMode, ClosestPkgJsonError> {
    if self.in_npm_pkg_checker.in_npm_package(specifier) {
      if let Some(pkg_json) =
        self.pkg_json_resolver.get_closest_package_json(specifier)?
      {
        let is_file_location_cjs = pkg_json.typ != "module";
        Ok(if is_file_location_cjs {
          ResolutionMode::Require
        } else {
          ResolutionMode::Import
        })
      } else {
        Ok(ResolutionMode::Require)
      }
    } else if self.mode != IsCjsResolutionMode::Disabled {
      if let Some(pkg_json) =
        self.pkg_json_resolver.get_closest_package_json(specifier)?
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
}
