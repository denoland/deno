// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::flags_rt::Flags;
use crate::version;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::futures::FutureExt;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::v8_set_flags;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::colors;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use std::cell::RefCell;
use std::convert::TryInto;
use std::env::current_exe;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::iter::once;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Deserialize, Serialize)]
pub(crate) struct Metadata {
  pub(crate) flags: Flags,
  pub(crate) ca_data: Option<Vec<u8>>,
}

const MAGIC_TRAILER: &[u8; 8] = b"d3n0l4nd";

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a stanalone
/// binary by checking for the magic trailer string `D3N0` at EOF-12.
/// The magic trailer is followed by:
/// - a u64 pointer to the JS bundle embedded in the binary
/// - a u64 pointer to JSON metadata (serialized flags) embedded in the binary
/// These are dereferenced, and the bundle is executed under the configuration
/// specified by the metadata. If no magic trailer is present, this function
/// exits with `Ok(())`.
pub fn try_run_standalone_binary(args: Vec<String>) -> Result<(), AnyError> {
  let current_exe_path = current_exe()?;

  let mut current_exe = File::open(current_exe_path)?;
  let trailer_pos = current_exe.seek(SeekFrom::End(-24))?;
  let mut trailer = [0; 24];
  current_exe.read_exact(&mut trailer)?;
  let (magic_trailer, rest) = trailer.split_at(8);
  if magic_trailer == MAGIC_TRAILER {
    let (bundle_pos, rest) = rest.split_at(8);
    let metadata_pos = rest;
    let bundle_pos = u64_from_bytes(bundle_pos)?;
    let metadata_pos = u64_from_bytes(metadata_pos)?;
    let bundle_len = metadata_pos - bundle_pos;
    let metadata_len = trailer_pos - metadata_pos;
    current_exe.seek(SeekFrom::Start(bundle_pos))?;

    let bundle = read_string_slice(&mut current_exe, bundle_pos, bundle_len)
      .context("Failed to read source bundle from the current executable")?;
    let metadata =
      read_string_slice(&mut current_exe, metadata_pos, metadata_len)
        .context("Failed to read metadata from the current executable")?;

    let mut metadata: Metadata = serde_json::from_str(&metadata).unwrap();
    metadata.flags.argv.append(&mut args[1..].to_vec());
    if let Err(err) = tokio_util::run_basic(run(bundle, metadata)) {
      eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
      std::process::exit(1);
    }
    std::process::exit(0);
  } else {
    Ok(())
  }
}

fn u64_from_bytes(arr: &[u8]) -> Result<u64, AnyError> {
  let fixed_arr: &[u8; 8] = arr
    .try_into()
    .context("Failed to convert the buffer into a fixed-size array")?;
  Ok(u64::from_be_bytes(*fixed_arr))
}

fn read_string_slice(
  file: &mut File,
  pos: u64,
  len: u64,
) -> Result<String, AnyError> {
  let mut string = String::new();
  file.seek(SeekFrom::Start(pos))?;
  file.take(len).read_to_string(&mut string)?;
  // TODO: check amount of bytes read
  Ok(string)
}

const SPECIFIER: &str = "file://$deno$/bundle.js";

struct EmbeddedModuleLoader(String);

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    _referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    if specifier != SPECIFIER {
      return Err(type_error(
        "Self-contained binaries don't support module loading",
      ));
    }
    Ok(ModuleSpecifier::resolve_url(specifier)?)
  }

  fn load(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.clone();
    let code = self.0.to_string();
    async move {
      if module_specifier.to_string() != SPECIFIER {
        return Err(type_error(
          "Self-contained binaries don't support module loading",
        ));
      }
      Ok(deno_core::ModuleSource {
        code,
        module_url_specified: module_specifier.to_string(),
        module_url_found: module_specifier.to_string(),
      })
    }
    .boxed_local()
  }
}

async fn run(source_code: String, metadata: Metadata) -> Result<(), AnyError> {
  let Metadata { flags, ca_data } = metadata;
  let main_module = ModuleSpecifier::resolve_url(SPECIFIER)?;
  let permissions = Permissions::from_options(&flags.clone().into());
  let module_loader = Rc::new(EmbeddedModuleLoader(source_code));
  let create_web_worker_cb = Arc::new(|_| {
    todo!("Worker are currently not supported in standalone binaries");
  });

  // Keep in sync with `main.rs`.
  v8_set_flags(
    once("UNUSED_BUT_NECESSARY_ARG0".to_owned())
      .chain(flags.v8_flags.iter().cloned())
      .collect::<Vec<_>>(),
  );
  // TODO(nayeemrmn): Unify this Flags -> WorkerOptions mapping with `deno run`.
  let options = WorkerOptions {
    apply_source_maps: false,
    args: flags.argv.clone(),
    debug_flag: flags.log_level.map_or(false, |l| l == log::Level::Debug),
    user_agent: format!("Deno/{}", version::deno()),
    unstable: flags.unstable,
    ca_data,
    seed: flags.seed,
    js_error_create_fn: None,
    create_web_worker_cb,
    attach_inspector: false,
    maybe_inspector_server: None,
    should_break_on_first_statement: false,
    module_loader,
    runtime_version: version::deno(),
    ts_version: version::TYPESCRIPT.to_string(),
    no_color: !colors::use_color(),
    get_error_class_fn: Some(&get_error_class_name),
  };
  let mut worker =
    MainWorker::from_options(main_module.clone(), permissions, &options);
  worker.bootstrap(&options);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  worker.run_event_loop().await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

fn get_error_class_name(e: &AnyError) -> &'static str {
  deno_runtime::errors::get_error_class_name(e).unwrap_or_else(|| {
    panic!("Error '{}' contains boxed error of unknown type", e);
  })
}
