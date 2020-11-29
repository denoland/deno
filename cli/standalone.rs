use crate::colors;
use crate::flags::Flags;
use crate::permissions::Permissions;
use crate::program_state::ProgramState;
use crate::tokio_util;
use crate::worker::MainWorker;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use std::cell::RefCell;
use std::convert::TryInto;
use std::env::current_exe;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::pin::Pin;
use std::rc::Rc;

pub fn standalone() {
  let current_exe_path =
    current_exe().expect("expect current exe path to be known");

  let mut current_exe = File::open(current_exe_path)
    .expect("expected to be able to open current exe");
  let magic_trailer_pos = current_exe
    .seek(SeekFrom::End(-12))
    .expect("expected to be able to seek to magic trailer in current exe");
  let mut magic_trailer = [0; 12];
  current_exe
    .read_exact(&mut magic_trailer)
    .expect("expected to be able to read magic trailer from current exe");
  let (magic_trailer, bundle_pos) = magic_trailer.split_at(4);
  if magic_trailer == b"DENO" {
    let bundle_pos_arr: &[u8; 8] =
      bundle_pos.try_into().expect("slice with incorrect length");
    let bundle_pos = u64::from_be_bytes(*bundle_pos_arr);
    current_exe
      .seek(SeekFrom::Start(bundle_pos))
      .expect("expected to be able to seek to bundle pos in current exe");

    let bundle_len = magic_trailer_pos - bundle_pos;
    let mut bundle = String::new();
    current_exe
      .take(bundle_len)
      .read_to_string(&mut bundle)
      .expect("expected to be able to read bundle from current exe");
    // TODO: check amount of bytes read

    let result = tokio_util::run_basic(run(bundle));
    if let Err(err) = result {
      eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
      std::process::exit(1);
    }
    std::process::exit(0);
  }
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
    assert_eq!(specifier, SPECIFIER);
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
      Ok(deno_core::ModuleSource {
        code,
        module_url_specified: module_specifier.to_string(),
        module_url_found: module_specifier.to_string(),
      })
    }
    .boxed_local()
  }
}

async fn run(source_code: String) -> Result<(), AnyError> {
  let flags = Flags::default();
  let main_module = ModuleSpecifier::resolve_url(SPECIFIER)?;
  let program_state = ProgramState::new(flags.clone())?;
  let permissions = Permissions::allow_all();
  let module_loader = Rc::new(EmbeddedModuleLoader(source_code));
  let mut worker = MainWorker::from_options(
    &program_state,
    main_module.clone(),
    permissions,
    module_loader,
  );
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  worker.run_event_loop().await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}
