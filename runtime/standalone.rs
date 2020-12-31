use crate::colors;
use crate::permissions::Permissions;
use crate::tokio_util;
use crate::worker::MainWorker;
use crate::worker::WorkerOptions;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use std::cell::RefCell;
use std::convert::TryInto;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

const MAGIC_TRAILER: &[u8; 8] = b"d3n0l4nd";
const SPECIFIER: &str = "file://$deno$/bundle.js";

pub fn try_run_standalone_binary(args: Vec<String>) -> Result<(), AnyError> {
  let current_exe_path = std::env::current_exe()?;

  let mut current_exe = File::open(current_exe_path)?;
  let trailer_pos = current_exe.seek(SeekFrom::End(-16))?;
  let mut trailer = [0; 16];
  current_exe.read_exact(&mut trailer)?;
  let (magic_trailer, bundle_pos_arr) = trailer.split_at(8);
  if magic_trailer == MAGIC_TRAILER {
    let bundle_pos_arr: &[u8; 8] = bundle_pos_arr.try_into()?;
    let bundle_pos = u64::from_be_bytes(*bundle_pos_arr);
    current_exe.seek(SeekFrom::Start(bundle_pos))?;

    let bundle_len = trailer_pos - bundle_pos;
    let mut bundle = String::new();
    current_exe.take(bundle_len).read_to_string(&mut bundle)?;
    // TODO: check amount of bytes read

    let parsed_args: Vec<String> = args[1..].to_vec();
    if let Err(err) = tokio_util::run_basic(run(bundle, parsed_args)) {
      eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
      std::process::exit(1);
    }
    std::process::exit(0);
  } else {
    Ok(())
  }
}

async fn run(source_code: String, args: Vec<String>) -> Result<(), AnyError> {
  let main_module = ModuleSpecifier::resolve_url(SPECIFIER)?;
  let permissions = Permissions::allow_all();
  let module_loader = Rc::new(EmbeddedModuleLoader(source_code));
  let create_web_worker_cb = Arc::new(|_| {
    todo!("Worker are currently not supported in standalone binaries");
  });

  let options = WorkerOptions {
    apply_source_maps: false,
    args,
    debug_flag: false,
    user_agent: "Deno/1.6.3".to_string(), // TODO (yos1p) Based on Deno version and user agent
    unstable: true,
    ca_filepath: None,
    seed: None,
    js_error_create_fn: None,
    create_web_worker_cb,
    attach_inspector: false,
    maybe_inspector_server: None,
    should_break_on_first_statement: false,
    module_loader,
    runtime_version: "1.6.3".to_string(), // TODO (yos1p) Deno version
    ts_version: "4.1.3".to_string(),      // TODO (yos1p) TS version
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
  crate::errors::get_error_class_name(e).unwrap_or_else(|| {
    panic!("Error '{}' contains boxed error of unknown type", e);
  })
}

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
