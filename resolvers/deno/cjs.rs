// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use dashmap::DashMap;
use deno_media_type::MediaType;
use node_resolver::env::NodeResolverEnv;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeModuleKind;
use node_resolver::PackageJsonResolver;
use url::Url;

/// Keeps track of what module specifiers were resolved as CJS.
///
/// Modules that are `.js`, `.ts`, `.jsx`, and `tsx` are only known to
/// be CJS or ESM after they're loaded based on their contents. So these
/// files will be "maybe CJS" until they're loaded.
#[derive(Debug)]
pub struct CjsTracker<TEnv: NodeResolverEnv> {
  is_cjs_resolver: IsCjsResolver<TEnv>,
  known: DashMap<Url, NodeModuleKind>,
}

impl<TEnv: NodeResolverEnv> CjsTracker<TEnv> {
  pub fn new(
    in_npm_pkg_checker: Arc<dyn InNpmPackageChecker>,
    pkg_json_resolver: Arc<PackageJsonResolver<TEnv>>,
    options: IsCjsResolverOptions,
  ) -> Self {
    Self {
      is_cjs_resolver: IsCjsResolver::new(
        in_npm_pkg_checker,
        pkg_json_resolver,
        options,
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
      .get_known_kind_with_is_script(specifier, media_type, is_script)
    {
      Some(kind) => kind,
      None => self.is_cjs_resolver.check_based_on_pkg_json(specifier)?,
    };
    Ok(kind == NodeModuleKind::Cjs)
  }

  /// Gets the referrer for the specified module specifier.
  ///
  /// Generally the referrer should already be tracked by calling
  /// `is_cjs_with_known_is_script` before calling this method.
  pub fn get_referrer_kind(&self, specifier: &Url) -> NodeModuleKind {
    if specifier.scheme() != "file" {
      return NodeModuleKind::Esm;
    }
    self
      .get_known_kind(specifier, MediaType::from_specifier(specifier))
      .unwrap_or(NodeModuleKind::Esm)
  }

  fn get_known_kind(
    &self,
    specifier: &Url,
    media_type: MediaType,
  ) -> Option<NodeModuleKind> {
    self.get_known_kind_with_is_script(specifier, media_type, None)
  }

  fn get_known_kind_with_is_script(
    &self,
    specifier: &Url,
    media_type: MediaType,
    is_script: Option<bool>,
  ) -> Option<NodeModuleKind> {
    self.is_cjs_resolver.get_known_kind_with_is_script(
      specifier,
      media_type,
      is_script,
      &self.known,
    )
  }
}

#[derive(Debug)]
pub struct IsCjsResolverOptions {
  pub detect_cjs: bool,
  pub is_node_main: bool,
}

/// Resolves whether a module is CJS or ESM.
#[derive(Debug)]
pub struct IsCjsResolver<TEnv: NodeResolverEnv> {
  in_npm_pkg_checker: Arc<dyn InNpmPackageChecker>,
  pkg_json_resolver: Arc<PackageJsonResolver<TEnv>>,
  options: IsCjsResolverOptions,
}

impl<TEnv: NodeResolverEnv> IsCjsResolver<TEnv> {
  pub fn new(
    in_npm_pkg_checker: Arc<dyn InNpmPackageChecker>,
    pkg_json_resolver: Arc<PackageJsonResolver<TEnv>>,
    options: IsCjsResolverOptions,
  ) -> Self {
    Self {
      in_npm_pkg_checker,
      pkg_json_resolver,
      options,
    }
  }

  /// Gets the referrer kind for a script in the LSP.
  pub fn get_lsp_referrer_kind(
    &self,
    specifier: &Url,
    is_script: Option<bool>,
  ) -> NodeModuleKind {
    if specifier.scheme() != "file" {
      return NodeModuleKind::Esm;
    }
    match MediaType::from_specifier(specifier) {
      MediaType::Mts | MediaType::Mjs | MediaType::Dmts => NodeModuleKind::Esm,
      MediaType::Cjs | MediaType::Cts | MediaType::Dcts => NodeModuleKind::Cjs,
      MediaType::Dts => {
        // dts files are always determined based on the package.json because
        // they contain imports/exports even when considered CJS
        self.check_based_on_pkg_json(specifier).unwrap_or(NodeModuleKind::Esm)
      }
      MediaType::Wasm |
      MediaType::Json => NodeModuleKind::Esm,
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::TypeScript
      | MediaType::Tsx
      // treat these as unknown
      | MediaType::Css
      | MediaType::SourceMap
      | MediaType::Unknown => {
        match is_script {
          Some(true) => self.check_based_on_pkg_json(specifier).unwrap_or(NodeModuleKind::Esm),
          Some(false) | None => NodeModuleKind::Esm,
        }
      }
    }
  }

  fn get_known_kind_with_is_script(
    &self,
    specifier: &Url,
    media_type: MediaType,
    is_script: Option<bool>,
    known_cache: &DashMap<Url, NodeModuleKind>,
  ) -> Option<NodeModuleKind> {
    if specifier.scheme() != "file" {
      return Some(NodeModuleKind::Esm);
    }

    match media_type {
      MediaType::Mts | MediaType::Mjs | MediaType::Dmts => Some(NodeModuleKind::Esm),
      MediaType::Cjs | MediaType::Cts | MediaType::Dcts => Some(NodeModuleKind::Cjs),
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
          Some(value.unwrap_or(NodeModuleKind::Esm))
        }
      }
      MediaType::Wasm |
      MediaType::Json => Some(NodeModuleKind::Esm),
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::TypeScript
      | MediaType::Tsx
      // treat these as unknown
      | MediaType::Css
      | MediaType::SourceMap
      | MediaType::Unknown => {
        if let Some(value) = known_cache.get(specifier).map(|v| *v) {
          if value == NodeModuleKind::Cjs && is_script == Some(false) {
            // we now know this is actually esm
            known_cache.insert(specifier.clone(), NodeModuleKind::Esm);
            Some(NodeModuleKind::Esm)
          } else {
            Some(value)
          }
        } else if is_script == Some(false) {
          // we know this is esm
            known_cache.insert(specifier.clone(), NodeModuleKind::Esm);
          Some(NodeModuleKind::Esm)
        } else {
          None
        }
      }
    }
  }

  fn check_based_on_pkg_json(
    &self,
    specifier: &Url,
  ) -> Result<NodeModuleKind, ClosestPkgJsonError> {
    if self.in_npm_pkg_checker.in_npm_package(specifier) {
      if let Some(pkg_json) =
        self.pkg_json_resolver.get_closest_package_json(specifier)?
      {
        let is_file_location_cjs = pkg_json.typ != "module";
        Ok(if is_file_location_cjs {
          NodeModuleKind::Cjs
        } else {
          NodeModuleKind::Esm
        })
      } else {
        Ok(NodeModuleKind::Cjs)
      }
    } else if self.options.detect_cjs || self.options.is_node_main {
      if let Some(pkg_json) =
        self.pkg_json_resolver.get_closest_package_json(specifier)?
      {
        let is_cjs_type = pkg_json.typ == "commonjs"
          || self.options.is_node_main && pkg_json.typ == "none";
        Ok(if is_cjs_type {
          NodeModuleKind::Cjs
        } else {
          NodeModuleKind::Esm
        })
      } else if self.options.is_node_main {
        Ok(NodeModuleKind::Cjs)
      } else {
        Ok(NodeModuleKind::Esm)
      }
    } else {
      Ok(NodeModuleKind::Esm)
    }
  }
}
