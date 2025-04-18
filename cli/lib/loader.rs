// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use deno_media_type::MediaType;
use deno_resolver::cjs::CjsTracker;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_runtime::deno_core::ModuleSourceCode;
use node_resolver::analyze::CjsCodeAnalyzer;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::InNpmPackageChecker;
use node_resolver::IsBuiltInNodeModuleChecker;
use node_resolver::NpmPackageFolderResolver;
use thiserror::Error;
use url::Url;

use crate::sys::DenoLibSys;
use crate::util::text_encoding::from_utf8_lossy_cow;

pub struct ModuleCodeStringSource {
  pub code: ModuleSourceCode,
  pub found_url: Url,
  pub media_type: MediaType,
}

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
  ClosestPkgJson(#[from] node_resolver::errors::ClosestPkgJsonError),
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
    if let Some(entrypoint_name) = suggestion {
      msg.push_str("\nDid you mean to import ");
      msg.push_str(entrypoint_name);
      msg.push_str(" within the directory?");
    }
  }
  msg
}

#[derive(Clone)]
pub struct NpmModuleLoader<
  TCjsCodeAnalyzer: CjsCodeAnalyzer,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: DenoLibSys,
> {
  cjs_tracker: Arc<CjsTracker<DenoInNpmPackageChecker, TSys>>,
  sys: TSys,
  node_code_translator: Arc<
    NodeCodeTranslator<
      TCjsCodeAnalyzer,
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
  >,
}

impl<
    TCjsCodeAnalyzer: CjsCodeAnalyzer,
    TInNpmPackageChecker: InNpmPackageChecker,
    TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver: NpmPackageFolderResolver,
    TSys: DenoLibSys,
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
    cjs_tracker: Arc<CjsTracker<DenoInNpmPackageChecker, TSys>>,
    node_code_translator: Arc<
      NodeCodeTranslator<
        TCjsCodeAnalyzer,
        TInNpmPackageChecker,
        TIsBuiltInNodeModuleChecker,
        TNpmPackageFolderResolver,
        TSys,
      >,
    >,
    sys: TSys,
  ) -> Self {
    Self {
      cjs_tracker,
      node_code_translator,
      sys,
    }
  }

  pub async fn load(
    &self,
    specifier: &Url,
    maybe_referrer: Option<&Url>,
  ) -> Result<ModuleCodeStringSource, NpmModuleLoadError> {
    let file_path = deno_path_util::url_to_file_path(specifier)?;
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

    let media_type = MediaType::from_specifier(specifier);
    if media_type.is_emittable() {
      return Err(NpmModuleLoadError::StrippingTypesNodeModules(
        StrippingTypesNodeModulesError {
          specifier: specifier.clone(),
        },
      ));
    }

    let code = if self.cjs_tracker.is_maybe_cjs(specifier, media_type)? {
      // translate cjs to esm if it's cjs and inject node globals
      let code = from_utf8_lossy_cow(code);
      ModuleSourceCode::String(
        self
          .node_code_translator
          .translate_cjs_to_esm(specifier, Some(code))
          .await?
          .into_owned()
          .into(),
      )
    } else {
      // esm and json code is untouched
      ModuleSourceCode::Bytes(match code {
        Cow::Owned(bytes) => bytes.into_boxed_slice().into(),
        Cow::Borrowed(bytes) => bytes.into(),
      })
    };

    Ok(ModuleCodeStringSource {
      code,
      found_url: specifier.clone(),
      media_type: MediaType::from_specifier(specifier),
    })
  }
}
