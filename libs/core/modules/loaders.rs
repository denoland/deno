// Copyright 2018-2025 the Deno authors. MIT license.

use crate::ModuleSourceCode;
use crate::error::CoreError;
use crate::error::CoreErrorKind;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::IntoModuleCodeString;
use crate::modules::ModuleCodeString;
use crate::modules::ModuleName;
use crate::modules::ModuleSource;
use crate::modules::ModuleSourceFuture;
use crate::modules::ModuleType;
use crate::modules::RequestedModuleType;
use crate::modules::ResolutionKind;
use crate::resolve_import;
use deno_error::JsErrorBox;

use futures::future::FutureExt;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use super::SourceCodeCacheInfo;

pub type ModuleLoaderError = JsErrorBox;

/// Result of calling `ModuleLoader::load`.
pub enum ModuleLoadResponse {
  /// Source file is available synchronously - eg. embedder might have
  /// collected all the necessary sources in `ModuleLoader::prepare_module_load`.
  /// Slightly cheaper than `Async` as it avoids boxing.
  Sync(Result<ModuleSource, ModuleLoaderError>),

  /// Source file needs to be loaded. Requires boxing due to recrusive
  /// nature of module loading.
  Async(Pin<Box<ModuleSourceFuture>>),
}

pub struct ModuleLoadOptions {
  pub is_dynamic_import: bool,
  /// If this is a synchronous ES module load.
  pub is_synchronous: bool,
  pub requested_module_type: RequestedModuleType,
}

#[derive(Debug, Clone)]
pub struct ModuleLoadReferrer {
  pub specifier: ModuleSpecifier,
  /// 1-based.
  pub line_number: i64,
  /// 1-based.
  pub column_number: i64,
}

pub trait ModuleLoader {
  /// Returns an absolute URL.
  /// When implementing an spec-complaint VM, this should be exactly the
  /// algorithm described here:
  /// <https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier>
  ///
  /// [`ResolutionKind::MainModule`] can be used to resolve from current working directory or
  /// apply import map for child imports.
  ///
  /// [`ResolutionKind::DynamicImport`] can be used to check permissions or deny
  /// dynamic imports altogether.
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError>;

  /// Override to customize the behavior of `import.meta.resolve` resolution.
  fn import_meta_resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    self.resolve(specifier, referrer, ResolutionKind::DynamicImport)
  }

  /// Given ModuleSpecifier, load its source code.
  ///
  /// `is_dyn_import` can be used to check permissions or deny
  /// dynamic imports altogether.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> ModuleLoadResponse;

  /// This hook can be used by implementors to do some preparation
  /// work before starting loading of modules.
  ///
  /// For example implementor might download multiple modules in
  /// parallel and transpile them to final JS sources before
  /// yielding control back to the runtime.
  ///
  /// It's not required to implement this method.
  fn prepare_load(
    &self,
    _module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    _maybe_content: Option<String>,
    _options: ModuleLoadOptions,
  ) -> Pin<Box<dyn Future<Output = Result<(), ModuleLoaderError>>>> {
    async { Ok(()) }.boxed_local()
  }

  /// This hook can be used by implementors to do some cleanup
  /// work after loading of modules. The hook is called for
  /// all loads, whether they succeeded or not.
  ///
  /// For example implementor might drop transpilation and
  /// static analysis caches before
  /// yielding control back to the runtime.
  ///
  /// It's not required to implement this method.
  fn finish_load(&self) {}

  /// Called when new v8 code cache is available for this module. Implementors
  /// can store the provided code cache for future executions of the same module.
  ///
  /// It's not required to implement this method.
  fn code_cache_ready(
    &self,
    _module_specifier: ModuleSpecifier,
    _hash: u64,
    _code_cache: &[u8],
  ) -> Pin<Box<dyn Future<Output = ()>>> {
    async {}.boxed_local()
  }

  /// Called when V8 code cache should be ignored for this module. This can happen
  /// if eg. module causes a V8 warning, like when using deprecated import assertions.
  /// Implementors should make sure that the code cache for this module is purged and not saved anymore.
  ///
  /// It's not required to implement this method.
  fn purge_and_prevent_code_cache(&self, _module_specifier: &str) {}

  /// Returns a source map for given `file_name`.
  ///
  /// This function will soon be deprecated or renamed.
  fn get_source_map(&self, _file_name: &str) -> Option<Cow<'_, [u8]>> {
    None
  }

  /// Loads an external source map file referenced by a module.
  fn load_external_source_map(
    &self,
    _source_map_url: &str,
  ) -> Option<Cow<'_, [u8]>> {
    None
  }

  /// Checks if a source file referenced in a source map exists. Used by the
  /// source map logic to verify that source files actually exist before
  /// rewriting stack trace file names.
  ///
  /// Returns `Some(true)` if the file exists, `Some(false)` if it doesn't,
  /// or `None` if existence cannot be determined.
  fn source_map_source_exists(&self, _source_url: &str) -> Option<bool> {
    None
  }

  fn get_source_mapped_source_line(
    &self,
    _file_name: &str,
    _line_number: usize,
  ) -> Option<String> {
    None
  }

  /// Implementors can attach arbitrary data to scripts and modules
  /// by implementing this method. V8 currently requires that the
  /// returned data be a `v8::PrimitiveArray`.
  fn get_host_defined_options<'s, 'i>(
    &self,
    _scope: &mut v8::PinScope<'s, 'i>,
    _name: &str,
  ) -> Option<v8::Local<'s, v8::Data>> {
    None
  }
}

/// Placeholder structure used when creating
/// a runtime that doesn't support module loading.
pub struct NoopModuleLoader;

impl ModuleLoader for NoopModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    _module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    ModuleLoadResponse::Sync(Err(JsErrorBox::generic(
      "Module loading is not supported.",
    )))
  }
}

pub trait ExtCodeCache {
  fn get_code_cache_info(
    &self,
    specifier: &ModuleSpecifier,
    code: &ModuleSourceCode,
    esm: bool,
  ) -> SourceCodeCacheInfo;

  fn code_cache_ready(
    &self,
    specifier: ModuleSpecifier,
    hash: u64,
    code_cache: &[u8],
    esm: bool,
  );
}

pub(crate) struct ExtModuleLoader {
  sources: RefCell<HashMap<ModuleName, ModuleCodeString>>,
  ext_code_cache: Option<Rc<dyn ExtCodeCache>>,
}

impl ExtModuleLoader {
  pub fn new(
    loaded_sources: Vec<(ModuleName, ModuleCodeString)>,
    ext_code_cache: Option<Rc<dyn ExtCodeCache>>,
  ) -> Self {
    // Guesstimate a length
    let mut sources = HashMap::with_capacity(loaded_sources.len());
    for source in loaded_sources {
      sources.insert(source.0, source.1);
    }
    ExtModuleLoader {
      sources: RefCell::new(sources),
      ext_code_cache,
    }
  }

  pub fn finalize(&self) -> Result<(), CoreError> {
    let sources = self.sources.take();
    let unused_modules: Vec<_> = sources.iter().collect();

    if !unused_modules.is_empty() {
      return Err(
        CoreErrorKind::UnusedModules(
          unused_modules
            .into_iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>(),
        )
        .into_box(),
      );
    }

    Ok(())
  }
}

impl ModuleLoader for ExtModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    // If specifier is relative to an extension module, we need to do some special handling
    if specifier.starts_with("../")
      || specifier.starts_with("./")
      || referrer.starts_with("ext:")
    {
      // add `/` to the referrer to make it a valid base URL, so we can join the specifier to it
      return crate::resolve_url(
        &crate::resolve_url(referrer.replace("ext:", "ext:/").as_str())
          .map_err(JsErrorBox::from_err)?
          .join(specifier)
          .map_err(crate::ModuleResolutionError::InvalidBaseUrl)
          .map_err(JsErrorBox::from_err)?
          .as_str()
          // remove the `/` we added
          .replace("ext:/", "ext:"),
      )
      .map_err(JsErrorBox::from_err);
    }
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let mut sources = self.sources.borrow_mut();
    let source = match sources.remove(specifier.as_str()) {
      Some(source) => source,
      None => {
        return ModuleLoadResponse::Sync(Err(JsErrorBox::generic(format!(
          "Specifier \"{0}\" was not passed as an extension module and was not included in the snapshot.",
          specifier
        ))));
      }
    };
    let code = ModuleSourceCode::String(source);
    let code_cache = self
      .ext_code_cache
      .as_ref()
      .map(|cache| cache.get_code_cache_info(specifier, &code, true));
    ModuleLoadResponse::Sync(Ok(ModuleSource::new(
      ModuleType::JavaScript,
      code,
      specifier,
      code_cache,
    )))
  }

  fn prepare_load(
    &self,
    _specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    _maybe_content: Option<String>,
    _options: ModuleLoadOptions,
  ) -> Pin<Box<dyn Future<Output = Result<(), ModuleLoaderError>>>> {
    async { Ok(()) }.boxed_local()
  }

  fn code_cache_ready(
    &self,
    module_specifier: ModuleSpecifier,
    hash: u64,
    code_cache: &[u8],
  ) -> Pin<Box<dyn Future<Output = ()>>> {
    if let Some(ext_code_cache) = &self.ext_code_cache {
      ext_code_cache.code_cache_ready(module_specifier, hash, code_cache, true);
    }
    std::future::ready(()).boxed_local()
  }
}

/// A loader that is used in `op_lazy_load_esm` to load and execute
/// ES modules that were embedded in the binary using `lazy_loaded_esm`
/// option in `extension!` macro.
pub(crate) struct LazyEsmModuleLoader {
  sources: Rc<RefCell<HashMap<ModuleName, ModuleCodeString>>>,
}

impl LazyEsmModuleLoader {
  pub fn new(
    sources: Rc<RefCell<HashMap<ModuleName, ModuleCodeString>>>,
  ) -> Self {
    LazyEsmModuleLoader { sources }
  }
}

impl ModuleLoader for LazyEsmModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let mut sources = self.sources.borrow_mut();
    let source = match sources.remove(specifier.as_str()) {
      Some(source) => source,
      None => {
        return ModuleLoadResponse::Sync(Err(JsErrorBox::generic(format!(
          "Specifier \"{0}\" cannot be lazy-loaded as it was not included in the binary.",
          specifier
        ))));
      }
    };
    ModuleLoadResponse::Sync(Ok(ModuleSource::new(
      ModuleType::JavaScript,
      ModuleSourceCode::String(source),
      specifier,
      None,
    )))
  }

  fn prepare_load(
    &self,
    _specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    _maybe_content: Option<String>,
    _options: ModuleLoadOptions,
  ) -> Pin<Box<dyn Future<Output = Result<(), ModuleLoaderError>>>> {
    async { Ok(()) }.boxed_local()
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("Failed to load {specifier}")]
pub struct LoadFailedError {
  specifier: ModuleSpecifier,
  #[source]
  #[inherit]
  source: std::io::Error,
}

/// Basic file system module loader.
///
/// Note that this loader will **block** event loop
/// when loading file as it uses synchronous FS API
/// from standard library.
pub struct FsModuleLoader;

impl ModuleLoader for FsModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();
    let fut = async move {
      let path = module_specifier.to_file_path().map_err(|_| {
        JsErrorBox::generic(format!(
          "Provided module specifier \"{module_specifier}\" is not a file URL."
        ))
      })?;
      let module_type = if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        // We only return JSON modules if extension was actually `.json`.
        // In other cases we defer to actual requested module type, so runtime
        // can decide what to do with it.
        if ext == "json" {
          ModuleType::Json
        } else if ext == "wasm" {
          ModuleType::Wasm
        } else {
          match &options.requested_module_type {
            RequestedModuleType::Other(ty) => ModuleType::Other(ty.clone()),
            RequestedModuleType::Text => ModuleType::Text,
            RequestedModuleType::Bytes => ModuleType::Bytes,
            _ => ModuleType::JavaScript,
          }
        }
      } else {
        ModuleType::JavaScript
      };

      // If we loaded a JSON file, but the "requested_module_type" (that is computed from
      // import attributes) is not JSON we need to fail.
      if module_type == ModuleType::Json
        && options.requested_module_type != RequestedModuleType::Json
      {
        return Err(JsErrorBox::generic("Attempted to load JSON module without specifying \"type\": \"json\" attribute in the import statement."));
      }

      let code = std::fs::read(path).map_err(|source| {
        JsErrorBox::from_err(LoadFailedError {
          specifier: module_specifier.clone(),
          source,
        })
      })?;
      let module = ModuleSource::new(
        module_type,
        ModuleSourceCode::Bytes(code.into_boxed_slice().into()),
        &module_specifier,
        None,
      );
      Ok(module)
    }
    .boxed_local();

    ModuleLoadResponse::Async(fut)
  }
}

/// A module loader that you can pre-load a number of modules into and resolve from. Useful for testing and
/// embedding situations where the filesystem and snapshot systems are not usable or a good fit.
#[derive(Default)]
pub struct StaticModuleLoader {
  map: HashMap<ModuleSpecifier, ModuleCodeString>,
}

impl StaticModuleLoader {
  /// Create a new [`StaticModuleLoader`] from an `Iterator` of specifiers and code.
  pub fn new(
    from: impl IntoIterator<Item = (ModuleSpecifier, impl IntoModuleCodeString)>,
  ) -> Self {
    Self {
      map: HashMap::from_iter(
        from.into_iter().map(|(url, code)| {
          (url, code.into_module_code().into_cheap_copy().0)
        }),
      ),
    }
  }

  /// Create a new [`StaticModuleLoader`] from a single code item.
  pub fn with(
    specifier: ModuleSpecifier,
    code: impl IntoModuleCodeString,
  ) -> Self {
    Self::new([(specifier, code)])
  }
}

impl ModuleLoader for StaticModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let res = if let Some(code) = self.map.get(module_specifier) {
      Ok(ModuleSource::new(
        ModuleType::JavaScript,
        ModuleSourceCode::String(code.try_clone().unwrap()),
        module_specifier,
        None,
      ))
    } else {
      Err(JsErrorBox::generic("Module not found"))
    };
    ModuleLoadResponse::Sync(res)
  }
}

/// Annotates a `ModuleLoader` with a log of all `load()` calls.
/// as well as a count of all `resolve()`, `prepare()`, and `load()` calls.
#[cfg(test)]
pub struct TestingModuleLoader<L: ModuleLoader> {
  loader: L,
  log: RefCell<Vec<ModuleSpecifier>>,
  load_count: std::cell::Cell<usize>,
  prepare_count: std::cell::Cell<usize>,
  finish_count: std::cell::Cell<usize>,
  resolve_count: std::cell::Cell<usize>,
}

#[cfg(test)]
impl<L: ModuleLoader> TestingModuleLoader<L> {
  pub fn new(loader: L) -> Self {
    Self {
      loader,
      log: RefCell::new(vec![]),
      load_count: Default::default(),
      prepare_count: Default::default(),
      finish_count: Default::default(),
      resolve_count: Default::default(),
    }
  }

  /// Retrieve the current module load event counts.
  pub fn counts(&self) -> ModuleLoadEventCounts {
    ModuleLoadEventCounts {
      load: self.load_count.get(),
      prepare: self.prepare_count.get(),
      finish: self.finish_count.get(),
      resolve: self.resolve_count.get(),
    }
  }
}

#[cfg(test)]
impl<L: ModuleLoader> ModuleLoader for TestingModuleLoader<L> {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    self.resolve_count.set(self.resolve_count.get() + 1);
    self.loader.resolve(specifier, referrer, kind)
  }

  fn prepare_load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<String>,
    maybe_content: Option<String>,
    options: ModuleLoadOptions,
  ) -> Pin<Box<dyn Future<Output = Result<(), ModuleLoaderError>>>> {
    self.prepare_count.set(self.prepare_count.get() + 1);
    self.loader.prepare_load(
      module_specifier,
      maybe_referrer,
      maybe_content,
      options,
    )
  }

  fn finish_load(&self) {
    self.finish_count.set(self.finish_count.get() + 1);
    self.loader.finish_load();
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    self.load_count.set(self.load_count.get() + 1);
    self.log.borrow_mut().push(module_specifier.clone());
    self.loader.load(module_specifier, maybe_referrer, options)
  }

  fn load_external_source_map(
    &self,
    source_map_url: &str,
  ) -> Option<Cow<'_, [u8]>> {
    self.loader.load_external_source_map(source_map_url)
  }
}

#[cfg(test)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct ModuleLoadEventCounts {
  pub resolve: usize,
  pub prepare: usize,
  pub finish: usize,
  pub load: usize,
}

#[cfg(test)]
impl ModuleLoadEventCounts {
  pub fn new(
    resolve: usize,
    prepare: usize,
    finish: usize,
    load: usize,
  ) -> Self {
    Self {
      resolve,
      prepare,
      finish,
      load,
    }
  }
}
