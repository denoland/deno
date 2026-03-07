// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::op2;

/// Shared resolve mappings between ops and the module loader.
/// Maps raw specifiers to resolved URLs.
#[derive(Clone, Default)]
pub struct ResolveMapping {
  pub map: Rc<RefCell<HashMap<String, String>>>,
}

/// Register a resolve mapping: when the loader sees `specifier`, it will
/// resolve it to `resolved` asynchronously.
#[op2(fast)]
pub fn op_loader_register_resolve(
  state: &mut OpState,
  #[string] specifier: String,
  #[string] resolved: String,
) {
  let mapping = state.borrow::<ResolveMapping>().clone();
  mapping.map.borrow_mut().insert(specifier, resolved);
}
