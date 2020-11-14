// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::permissions::Permissions;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;

pub struct FsModuleLoader;

impl ModuleLoader for FsModuleLoader {
  fn resolve(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    Ok(ModuleSpecifier::resolve_import(specifier, referrer)?)
  }

  fn load(
    &self,
    op_state: Rc<RefCell<OpState>>,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<ModuleSpecifier>,
    is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.clone();
    async move {
      if is_dynamic {
        let state = op_state.borrow();
        let dynamic_permissions = state.borrow::<Permissions>().clone();
        dynamic_permissions.check_specifier(&module_specifier)?;
      }
      let path = module_specifier.as_url().to_file_path().unwrap();
      let content = std::fs::read_to_string(path)?;
      let module = deno_core::ModuleSource {
        code: content,
        module_url_specified: module_specifier.to_string(),
        module_url_found: module_specifier.to_string(),
      };
      Ok(module)
    }
    .boxed_local()
  }
}
