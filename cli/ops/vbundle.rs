// Copyright 2018-2026 the Deno authors. MIT license.

//! Ops for the vbundle plugin system.
//!
//! These ops are used by the JavaScript plugin runtime to communicate
//! with the Rust bundler.

use deno_core::OpState;
use deno_core::op2;

use crate::tools::vbundle::PluginLogger;

deno_core::extension!(
  deno_vbundle_ext,
  ops = [
    op_vbundle_print,
    op_vbundle_emit_file,
  ],
  options = {
    logger: PluginLogger,
  },
  middleware = |op| match op.name {
    "op_print" => op_vbundle_print(),
    _ => op,
  },
  state = |state, options| {
    state.put(options.logger);
    state.put(VbundlePluginContainer::default());
  },
);

/// Container for vbundle plugin state.
#[derive(Default)]
pub struct VbundlePluginContainer {
  /// Emitted files from plugins.
  pub emitted_files: Vec<EmittedFile>,
}

/// An emitted file from a plugin.
#[derive(Debug, Clone)]
pub struct EmittedFile {
  /// The file type (chunk or asset).
  pub file_type: String,
  /// The file name.
  pub file_name: Option<String>,
  /// The source content.
  pub source: Option<String>,
}

/// Print operation for plugin logging.
#[op2(fast)]
pub fn op_vbundle_print(
  state: &mut OpState,
  #[string] msg: &str,
  is_err: bool,
) {
  let logger = state.borrow::<PluginLogger>();
  if is_err {
    logger.error(msg);
  } else {
    logger.log(msg);
  }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmitFileArgs {
  #[serde(rename = "type")]
  file_type: String,
  file_name: Option<String>,
  name: Option<String>,
  source: Option<String>,
}

/// Emit a file from a plugin (for assets, additional chunks, etc.).
#[op2]
#[string]
fn op_vbundle_emit_file(
  state: &mut OpState,
  #[serde] args: EmitFileArgs,
) -> String {
  let container = state.borrow_mut::<VbundlePluginContainer>();

  // Generate a reference ID for the emitted file
  let reference_id = format!("__EMITTED_{}__", container.emitted_files.len());

  container.emitted_files.push(EmittedFile {
    file_type: args.file_type,
    file_name: args.file_name.or(args.name),
    source: args.source,
  });

  reference_id
}
