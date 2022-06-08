// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::proc_state::ProcState;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;

pub mod bench;
pub mod testing;

pub fn cli_exts(ps: ProcState) -> Vec<Extension> {
  vec![init_proc_state(ps)]
}

fn init_proc_state(ps: ProcState) -> Extension {
  let is_file_watcher = ps.flags.watch.is_some();

  let mut ext = Extension::builder();

  ext.state(move |state| {
    state.put(ps.clone());
    Ok(())
  });

  if is_file_watcher {
    ext.middleware(|op| match op.name {
      "op_exit" => op_exit::decl(),
      _ => op,
    });
  }

  ext.build()
}

#[op]
fn op_exit(state: &mut OpState) {
  let code = state.borrow::<Arc<AtomicI32>>().load(Relaxed);
  eprintln!("I should have exited with code {}", code);
  std::process::exit(code);
}
