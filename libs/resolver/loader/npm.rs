// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;

use deno_media_type::MediaType;
use node_resolver::InNpmPackageChecker;
use node_resolver::IsBuiltInNodeModuleChecker;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::analyze::CjsCodeAnalyzer;
use node_resolver::analyze::NodeCodeTranslatorRc;
use node_resolver::analyze::NodeCodeTranslatorSys;
use thiserror::Error;
use url::Url;

use super::LoadedModule;
use super::LoadedModuleSource;
use super::RequestedModuleType;
use crate::cjs::CjsTrackerRc;

#[derive(Debug, Error, deno_error::JsError)]
#[class(type)]
#[error("[{}]: Stripping types is currently unsupported for files under node_modules, for \"{}\"", self.code(), specifier)]
pub struct StrippingTypesNodeModulesError {
  pub specifier: Url,
}

impl StrippingTypesNodeModulesError {
  pub fn code(&self) -> &'static str {
    "ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING"
  }
}

#[derive(Debug, Error, deno_error::JsError)]
pub enum NpmModuleLoadError {
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
  #[class(inherit)]
  #[error(transparent)]
  StrippingTypesNodeModules(#[from] StrippingTypesNodeModulesError),
  #[class(inherit)]
  #[error(transparent)]
  ClosestPkgJson(#[from] node_resolver::errors::PackageJsonLoadError),
  #[class(inherit)]
  #[error(transparent)]
  TranslateCjsToEsm(#[from] node_resolver::analyze::TranslateCjsToEsmError),
  #[class(inherit)]
  #[error("Unable to load {}{}", file_path.display(), maybe_referrer.as_ref().map(|r| format!(" imported from {}", r)).unwrap_or_default())]
  UnableToLoad {
    file_path: PathBuf,
    maybe_referrer: Option<Url>,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(
    "{}",
    format_dir_import_message(file_path, maybe_referrer, suggestion)
  )]
  DirImport {
    file_path: PathBuf,
    maybe_referrer: Option<Url>,
    suggestion: Option<&'static str>,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
}

fn format_dir_import_message(
  file_path: &std::path::Path,
  maybe_referrer: &Option<Url>,
  suggestion: &Option<&'static str>,
) -> String {
  // directory imports are not allowed when importing from an
  // ES module, so provide the user with a helpful error message
  let dir_path = file_path;
  let mut msg = "Directory import ".to_string();
  msg.push_str(&dir_path.to_string_lossy());
  if let Some(referrer) = maybe_referrer {
    msg.push_str(" is not supported resolving import from ");
    msg.push_str(referrer.as_str());
  }
  if let Some(entrypoint_name) = suggestion {
    msg.push_str("\nDid you mean to import ");
    msg.push_str(entrypoint_name);
    msg.push_str(" within the directory?");
  }
  msg
}

#[sys_traits::auto_impl]
pub trait NpmModuleLoaderSys: NodeCodeTranslatorSys {}

#[allow(clippy::disallowed_types)]
pub type DenoNpmModuleLoaderRc<TSys> =
  deno_maybe_sync::MaybeArc<DenoNpmModuleLoader<TSys>>;

pub type DenoNpmModuleLoader<TSys> = NpmModuleLoader<
  crate::cjs::analyzer::DenoCjsCodeAnalyzer<TSys>,
  crate::npm::DenoInNpmPackageChecker,
  node_resolver::DenoIsBuiltInNodeModuleChecker,
  crate::npm::NpmResolver<TSys>,
  TSys,
>;

#[derive(Clone)]
pub struct NpmModuleLoader<
  TCjsCodeAnalyzer: CjsCodeAnalyzer,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: NpmModuleLoaderSys,
> {
  cjs_tracker: CjsTrackerRc<TInNpmPackageChecker, TSys>,
  node_code_translator: NodeCodeTranslatorRc<
    TCjsCodeAnalyzer,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  sys: TSys,
}

impl<
  TCjsCodeAnalyzer: CjsCodeAnalyzer,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: NpmModuleLoaderSys,
>
  NpmModuleLoader<
    TCjsCodeAnalyzer,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  pub fn new(
    cjs_tracker: CjsTrackerRc<TInNpmPackageChecker, TSys>,
    node_code_translator: NodeCodeTranslatorRc<
      TCjsCodeAnalyzer,
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
    sys: TSys,
  ) -> Self {
    Self {
      cjs_tracker,
      node_code_translator,
      sys,
    }
  }

  pub async fn load<'a>(
    &self,
    specifier: Cow<'a, Url>,
    maybe_referrer: Option<&Url>,
    requested_module_type: &RequestedModuleType<'_>,
  ) -> Result<LoadedModule<'a>, NpmModuleLoadError> {
    let file_path = deno_path_util::url_to_file_path(&specifier)?;
    let code = self.sys.fs_read(&file_path).map_err(|source| {
      if self.sys.fs_is_dir_no_err(&file_path) {
        let suggestion = ["index.mjs", "index.js", "index.cjs"]
          .into_iter()
          .find(|e| self.sys.fs_is_file_no_err(file_path.join(e)));
        NpmModuleLoadError::DirImport {
          file_path,
          maybe_referrer: maybe_referrer.cloned(),
          suggestion,
          source,
        }
      } else {
        NpmModuleLoadError::UnableToLoad {
          file_path,
          maybe_referrer: maybe_referrer.cloned(),
          source,
        }
      }
    })?;

    let media_type = MediaType::from_specifier(&specifier);
    match requested_module_type {
      RequestedModuleType::Text | RequestedModuleType::Bytes => {
        Ok(LoadedModule {
          specifier,
          media_type,
          source: LoadedModuleSource::Bytes(code),
        })
      }
      RequestedModuleType::None
      | RequestedModuleType::Json
      | RequestedModuleType::Other(_) => {
        if media_type.is_emittable() {
          return Err(NpmModuleLoadError::StrippingTypesNodeModules(
            StrippingTypesNodeModulesError {
              specifier: specifier.into_owned(),
            },
          ));
        }

        let source = if self.cjs_tracker.is_maybe_cjs(&specifier, media_type)? {
          // translate cjs to esm if it's cjs and inject node globals
          let code = from_utf8_lossy_cow(code);
          LoadedModuleSource::String(
            self
              .node_code_translator
              .translate_cjs_to_esm(&specifier, Some(code))
              .await?
              .into_owned()
              .into(),
          )
        } else {
          // esm and json code is untouched
          LoadedModuleSource::Bytes(code)
        };

        Ok(LoadedModule {
          source,
          specifier,
          media_type,
        })
      }
    }
  }
}

#[inline(always)]
fn from_utf8_lossy_cow(bytes: Cow<'_, [u8]>) -> Cow<'_, str> {
  match bytes {
    Cow::Borrowed(bytes) => String::from_utf8_lossy(bytes),
    Cow::Owned(bytes) => Cow::Owned(from_utf8_lossy_owned(bytes)),
  }
}

// todo(https://github.com/rust-lang/rust/issues/129436): remove once stabilized
#[inline(always)]
fn from_utf8_lossy_owned(bytes: Vec<u8>) -> String {
  match String::from_utf8_lossy(&bytes) {
    Cow::Owned(code) => code,
    // SAFETY: `String::from_utf8_lossy` guarantees that the result is valid
    // UTF-8 if `Cow::Borrowed` is returned.
    Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(bytes) },
  }
}
