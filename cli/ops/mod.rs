// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::proc_state::ProcState;
use deno_core::Extension;

pub mod bench;
mod errors;
pub mod testing;

pub fn cli_exts(ps: ProcState) -> Vec<Extension> {
  vec![init_proc_state(ps), errors::init()]
}

fn init_proc_state(ps: ProcState) -> Extension {
  Extension::builder()
    .state(move |state| {
      state.put(ps.clone());
      Ok(())
    })
    .build()
}
