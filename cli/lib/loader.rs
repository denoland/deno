// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::sync::Arc;

use deno_media_type::MediaType;
use deno_resolver::cjs::CjsTracker;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_runtime::deno_core::anyhow::Context;
use deno_runtime::deno_core::error::AnyError;
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
#[error("{media_type} files are not supported in npm packages: {specifier}")]
pub struct NotSupportedKindInNpmError {
  pub media_type: MediaType,
  pub specifier: Url,
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
  ) -> Result<ModuleCodeStringSource, AnyError> {
    let file_path = deno_path_util::url_to_file_path(specifier)?;
    let code = self
      .sys
      .fs_read(&file_path)
      .map_err(AnyError::from)
      .with_context(|| {
        if self.sys.fs_is_dir_no_err(&file_path) {
          // directory imports are not allowed when importing from an
          // ES module, so provide the user with a helpful error message
          let dir_path = file_path;
          let mut msg = "Directory import ".to_string();
          msg.push_str(&dir_path.to_string_lossy());
          if let Some(referrer) = &maybe_referrer {
            msg.push_str(" is not supported resolving import from ");
            msg.push_str(referrer.as_str());
            let entrypoint_name = ["index.mjs", "index.js", "index.cjs"]
              .iter()
              .find(|e| self.sys.fs_is_file_no_err(dir_path.join(e)));
            if let Some(entrypoint_name) = entrypoint_name {
              msg.push_str("\nDid you mean to import ");
              msg.push_str(entrypoint_name);
              msg.push_str(" within the directory?");
            }
          }
          msg
        } else {
          let mut msg = "Unable to load ".to_string();
          msg.push_str(&file_path.to_string_lossy());
          if let Some(referrer) = &maybe_referrer {
            msg.push_str(" imported from ");
            msg.push_str(referrer.as_str());
          }
          msg
        }
      })?;

    let media_type = MediaType::from_specifier(specifier);
    if media_type.is_emittable() {
      return Err(AnyError::from(NotSupportedKindInNpmError {
        media_type,
        specifier: specifier.clone(),
      }));
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
