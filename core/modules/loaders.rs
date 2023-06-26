// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::error::generic_error;
use crate::error::AnyError;
use crate::extensions::ExtensionFileSource;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::ModuleCode;
use crate::modules::ModuleSource;
use crate::modules::ModuleSourceFuture;
use crate::modules::ModuleType;
use crate::modules::ResolutionKind;
use crate::resolve_import;
use crate::Extension;
use anyhow::anyhow;
use anyhow::Error;
use futures::future::FutureExt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub trait ModuleLoader {
  /// Returns an absolute URL.
  /// When implementing an spec-complaint VM, this should be exactly the
  /// algorithm described here:
  /// <https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier>
  ///
  /// `is_main` can be used to resolve from current working directory or
  /// apply import map for child imports.
  ///
  /// `is_dyn_import` can be used to check permissions or deny
  /// dynamic imports altogether.
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, Error>;

  /// Given ModuleSpecifier, load its source code.
  ///
  /// `is_dyn_import` can be used to check permissions or deny
  /// dynamic imports altogether.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dyn_import: bool,
  ) -> Pin<Box<ModuleSourceFuture>>;

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
    _is_dyn_import: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
    async { Ok(()) }.boxed_local()
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
  ) -> Result<ModuleSpecifier, Error> {
    Err(generic_error(
      format!("Module loading is not supported; attempted to resolve: \"{specifier}\" from \"{referrer}\"")
    ))
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
  ) -> Pin<Box<ModuleSourceFuture>> {
    let err = generic_error(
      format!(
        "Module loading is not supported; attempted to load: \"{module_specifier}\" from \"{maybe_referrer:?}\"",
      )
    );
    async move { Err(err) }.boxed_local()
  }
}

/// Function that can be passed to the `ExtModuleLoader` that allows to
/// transpile sources before passing to V8.
pub type ExtModuleLoaderCb =
  Box<dyn Fn(&ExtensionFileSource) -> Result<ModuleCode, Error>>;

pub(crate) struct ExtModuleLoader {
  maybe_load_callback: Option<Rc<ExtModuleLoaderCb>>,
  sources: RefCell<HashMap<String, ExtensionFileSource>>,
  used_specifiers: RefCell<HashSet<String>>,
}

impl ExtModuleLoader {
  pub fn new(
    extensions: &[Extension],
    maybe_load_callback: Option<Rc<ExtModuleLoaderCb>>,
  ) -> Self {
    let mut sources = HashMap::new();
    sources.extend(
      extensions
        .iter()
        .flat_map(|e| e.get_esm_sources())
        .flatten()
        .map(|s| (s.specifier.to_string(), s.clone())),
    );
    ExtModuleLoader {
      maybe_load_callback,
      sources: RefCell::new(sources),
      used_specifiers: Default::default(),
    }
  }
}

impl ModuleLoader for ExtModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, Error> {
    Ok(resolve_import(specifier, referrer)?)
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
  ) -> Pin<Box<ModuleSourceFuture>> {
    let sources = self.sources.borrow();
    let source = match sources.get(specifier.as_str()) {
      Some(source) => source,
      None => return futures::future::err(anyhow!("Specifier \"{}\" was not passed as an extension module and was not included in the snapshot.", specifier)).boxed_local(),
    };
    self
      .used_specifiers
      .borrow_mut()
      .insert(specifier.to_string());
    let result = if let Some(load_callback) = &self.maybe_load_callback {
      load_callback(source)
    } else {
      source.load()
    };
    match result {
      Ok(code) => {
        let res = ModuleSource::new(ModuleType::JavaScript, code, specifier);
        return futures::future::ok(res).boxed_local();
      }
      Err(err) => return futures::future::err(err).boxed_local(),
    }
  }

  fn prepare_load(
    &self,
    _specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    _is_dyn_import: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), Error>>>> {
    async { Ok(()) }.boxed_local()
  }
}

impl Drop for ExtModuleLoader {
  fn drop(&mut self) {
    let sources = self.sources.get_mut();
    let used_specifiers = self.used_specifiers.get_mut();
    let unused_modules: Vec<_> = sources
      .iter()
      .filter(|(k, _)| !used_specifiers.contains(k.as_str()))
      .collect();

    if !unused_modules.is_empty() {
      let mut msg =
        "Following modules were passed to ExtModuleLoader but never used:\n"
          .to_string();
      for m in unused_modules {
        msg.push_str("  - ");
        msg.push_str(m.0);
        msg.push('\n');
      }
      panic!("{}", msg);
    }
  }
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
  ) -> Result<ModuleSpecifier, Error> {
    Ok(resolve_import(specifier, referrer)?)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<ModuleSourceFuture>> {
    fn load(
      module_specifier: &ModuleSpecifier,
    ) -> Result<ModuleSource, AnyError> {
      let path = module_specifier.to_file_path().map_err(|_| {
        generic_error(format!(
          "Provided module specifier \"{module_specifier}\" is not a file URL."
        ))
      })?;
      let module_type = if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        if ext == "json" {
          ModuleType::Json
        } else {
          ModuleType::JavaScript
        }
      } else {
        ModuleType::JavaScript
      };

      let code = std::fs::read_to_string(path)?.into();
      let module = ModuleSource::new(module_type, code, module_specifier);
      Ok(module)
    }

    futures::future::ready(load(module_specifier)).boxed_local()
  }
}
