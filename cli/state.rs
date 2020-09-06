// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::errors::get_error_class_name;
use crate::file_fetcher::SourceFileFetcher;
use crate::global_state::GlobalState;
use crate::global_timer::GlobalTimer;
use crate::http_util::create_http_client;
use crate::import_map::ImportMap;
use crate::metrics::Metrics;
use crate::permissions::Permissions;
use crate::tsc::TargetLib;
use crate::web_worker::WebWorkerHandle;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::Op;
use deno_core::OpId;
use deno_core::OpRegistry;
use deno_core::OpRouter;
use deno_core::OpTable;
use deno_core::ResourceTable;
use futures::future::FutureExt;
use futures::Future;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct State {
  pub global_state: Arc<GlobalState>,
  pub permissions: RefCell<Permissions>,
  pub main_module: ModuleSpecifier,
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub metrics: RefCell<Metrics>,
  pub global_timer: RefCell<GlobalTimer>,
  pub workers: RefCell<HashMap<u32, (JoinHandle<()>, WebWorkerHandle)>>,
  pub next_worker_id: Cell<u32>,
  pub start_time: Instant,
  pub seeded_rng: Option<RefCell<StdRng>>,
  pub target_lib: TargetLib,
  pub is_main: bool,
  pub is_internal: bool,
  pub http_client: RefCell<reqwest::Client>,
  pub resource_table: RefCell<ResourceTable>,
  pub op_table: RefCell<OpTable<Self>>,
}

impl State {
  /// Quits the process if the --unstable flag was not provided.
  ///
  /// This is intentionally a non-recoverable check so that people cannot probe
  /// for unstable APIs from stable programs.
  pub fn check_unstable(&self, api_name: &str) {
    // TODO(ry) Maybe use IsolateHandle::terminate_execution here to provide a
    // stack trace in JS.
    if !self.global_state.flags.unstable {
      exit_unstable(api_name);
    }
  }
}

pub fn exit_unstable(api_name: &str) {
  eprintln!(
    "Unstable API '{}'. The --unstable flag must be provided.",
    api_name
  );
  std::process::exit(70);
}

impl ModuleLoader for State {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    is_main: bool,
  ) -> Result<ModuleSpecifier, ErrBox> {
    if !is_main {
      if let Some(import_map) = &self.import_map {
        let result = import_map.resolve(specifier, referrer)?;
        if let Some(r) = result {
          return Ok(r);
        }
      }
    }
    let module_specifier =
      ModuleSpecifier::resolve_import(specifier, referrer)?;

    Ok(module_specifier)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    _is_dyn_import: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.to_owned();
    // TODO(bartlomieju): incrementing resolve_count here has no sense...
    self.metrics.borrow_mut().resolve_count += 1;
    let module_url_specified = module_specifier.to_string();
    let global_state = self.global_state.clone();

    // TODO(bartlomieju): `fetch_compiled_module` should take `load_id` param
    let fut = async move {
      let compiled_module = global_state
        .fetch_compiled_module(module_specifier, maybe_referrer)
        .await?;
      Ok(deno_core::ModuleSource {
        // Real module name, might be different from initial specifier
        // due to redirections.
        code: compiled_module.code,
        module_url_specified,
        module_url_found: compiled_module.name,
      })
    };

    fut.boxed_local()
  }

  fn prepare_load(
    &self,
    _load_id: ModuleLoadId,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<String>,
    is_dyn_import: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), ErrBox>>>> {
    let module_specifier = module_specifier.clone();
    let target_lib = self.target_lib.clone();
    let maybe_import_map = self.import_map.clone();
    // Only "main" module is loaded without permission check,
    // ie. module that is associated with "is_main" state
    // and is not a dynamic import.
    let permissions = if self.is_main && !is_dyn_import {
      Permissions::allow_all()
    } else {
      self.permissions.borrow().clone()
    };
    let global_state = self.global_state.clone();
    // TODO(bartlomieju): I'm not sure if it's correct to ignore
    // bad referrer - this is the case for `Deno.core.evalContext()` where
    // `ref_str` is `<unknown>`.
    let maybe_referrer = if let Some(ref_str) = maybe_referrer {
      ModuleSpecifier::resolve_url(&ref_str).ok()
    } else {
      None
    };

    // TODO(bartlomieju): `prepare_module_load` should take `load_id` param
    async move {
      global_state
        .prepare_module_load(
          module_specifier,
          maybe_referrer,
          target_lib,
          permissions,
          is_dyn_import,
          maybe_import_map,
        )
        .await
    }
    .boxed_local()
  }
}

impl State {
  /// If `shared_permission` is None then permissions from globa state are used.
  pub fn new(
    global_state: &Arc<GlobalState>,
    shared_permissions: Option<Permissions>,
    main_module: ModuleSpecifier,
    maybe_import_map: Option<ImportMap>,
    is_internal: bool,
  ) -> Result<Rc<Self>, ErrBox> {
    let fl = &global_state.flags;
    let state = State {
      global_state: global_state.clone(),
      main_module,
      permissions: shared_permissions
        .unwrap_or_else(|| global_state.permissions.clone())
        .into(),
      import_map: maybe_import_map,
      metrics: Default::default(),
      global_timer: Default::default(),
      workers: Default::default(),
      next_worker_id: Default::default(),
      start_time: Instant::now(),
      seeded_rng: fl.seed.map(|v| StdRng::seed_from_u64(v).into()),
      target_lib: TargetLib::Main,
      is_main: true,
      is_internal,
      http_client: create_http_client(fl.ca_file.as_deref())?.into(),
      resource_table: Default::default(),
      op_table: Default::default(),
    };
    Ok(Rc::new(state))
  }

  /// If `shared_permission` is None then permissions from globa state are used.
  pub fn new_for_worker(
    global_state: &Arc<GlobalState>,
    shared_permissions: Option<Permissions>,
    main_module: ModuleSpecifier,
  ) -> Result<Rc<Self>, ErrBox> {
    let fl = &global_state.flags;
    let state = State {
      global_state: global_state.clone(),
      main_module,
      permissions: shared_permissions
        .unwrap_or_else(|| global_state.permissions.clone())
        .into(),
      import_map: None,
      metrics: Default::default(),
      global_timer: Default::default(),
      workers: Default::default(),
      next_worker_id: Default::default(),
      start_time: Instant::now(),
      seeded_rng: fl.seed.map(|v| StdRng::seed_from_u64(v).into()),
      target_lib: TargetLib::Worker,
      is_main: false,
      is_internal: false,
      http_client: create_http_client(fl.ca_file.as_deref())?.into(),
      resource_table: Default::default(),
      op_table: Default::default(),
    };
    Ok(Rc::new(state))
  }

  #[inline]
  pub fn check_read(&self, path: &Path) -> Result<(), ErrBox> {
    self.permissions.borrow().check_read(path)
  }

  /// As `check_read()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  #[inline]
  pub fn check_read_blind(
    &self,
    path: &Path,
    display: &str,
  ) -> Result<(), ErrBox> {
    self.permissions.borrow().check_read_blind(path, display)
  }

  #[inline]
  pub fn check_write(&self, path: &Path) -> Result<(), ErrBox> {
    self.permissions.borrow().check_write(path)
  }

  #[inline]
  pub fn check_env(&self) -> Result<(), ErrBox> {
    self.permissions.borrow().check_env()
  }

  #[inline]
  pub fn check_net(&self, hostname: &str, port: u16) -> Result<(), ErrBox> {
    self.permissions.borrow().check_net(hostname, port)
  }

  #[inline]
  pub fn check_net_url(&self, url: &url::Url) -> Result<(), ErrBox> {
    self.permissions.borrow().check_net_url(url)
  }

  #[inline]
  pub fn check_run(&self) -> Result<(), ErrBox> {
    self.permissions.borrow().check_run()
  }

  #[inline]
  pub fn check_hrtime(&self) -> Result<(), ErrBox> {
    self.permissions.borrow().check_hrtime()
  }

  #[inline]
  pub fn check_plugin(&self, filename: &Path) -> Result<(), ErrBox> {
    self.permissions.borrow().check_plugin(filename)
  }

  pub fn check_dyn_import(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), ErrBox> {
    let u = module_specifier.as_url();
    // TODO(bartlomieju): temporary fix to prevent hitting `unreachable`
    // statement that is actually reachable...
    SourceFileFetcher::check_if_supported_scheme(u)?;

    match u.scheme() {
      "http" | "https" => {
        self.check_net_url(u)?;
        Ok(())
      }
      "file" => {
        let path = u
          .to_file_path()
          .unwrap()
          .into_os_string()
          .into_string()
          .unwrap();
        self.check_read(Path::new(&path))?;
        Ok(())
      }
      _ => unreachable!(),
    }
  }

  #[cfg(test)]
  pub fn mock(main_module: &str) -> Rc<Self> {
    let module_specifier = ModuleSpecifier::resolve_url_or_path(main_module)
      .expect("Invalid entry module");
    State::new(
      &GlobalState::mock(vec!["deno".to_string()], None),
      None,
      module_specifier,
      None,
      false,
    )
    .unwrap()
  }
}

impl OpRouter for State {
  fn route_op(self: Rc<Self>, op_id: OpId, bufs: BufVec) -> Op {
    // TODOs:
    // * The 'bytes' metrics seem pretty useless, especially now that the
    //   distinction between 'control' and 'data' buffers has become blurry.
    // * Tracking completion of async ops currently makes us put the boxed
    //   future into _another_ box. Keeping some counters may not be expensive
    //   in itself, but adding a heap allocation for every metric seems bad.
    let mut buf_len_iter = bufs.iter().map(|buf| buf.len());
    let bytes_sent_control = buf_len_iter.next().unwrap_or(0);
    let bytes_sent_data = buf_len_iter.sum();

    let op_fn = self
      .op_table
      .borrow()
      .get_index(op_id)
      .map(|(_, op_fn)| op_fn.clone())
      .unwrap();

    let self_ = self.clone();
    let op = (op_fn)(self_, bufs);

    let self_ = self.clone();
    let mut metrics = self_.metrics.borrow_mut();
    match op {
      Op::Sync(buf) => {
        metrics.op_sync(bytes_sent_control, bytes_sent_data, buf.len());
        Op::Sync(buf)
      }
      Op::Async(fut) => {
        metrics.op_dispatched_async(bytes_sent_control, bytes_sent_data);
        let fut = fut
          .inspect(move |buf| {
            self.metrics.borrow_mut().op_completed_async(buf.len());
          })
          .boxed_local();
        Op::Async(fut)
      }
      Op::AsyncUnref(fut) => {
        metrics.op_dispatched_async_unref(bytes_sent_control, bytes_sent_data);
        let fut = fut
          .inspect(move |buf| {
            self
              .metrics
              .borrow_mut()
              .op_completed_async_unref(buf.len());
          })
          .boxed_local();
        Op::AsyncUnref(fut)
      }
      other => other,
    }
  }
}

impl OpRegistry for State {
  fn get_op_catalog(self: Rc<Self>) -> HashMap<String, OpId> {
    self.op_table.borrow().get_op_catalog()
  }

  fn register_op<F>(&self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, BufVec) -> Op + 'static,
  {
    let mut op_table = self.op_table.borrow_mut();
    let (op_id, prev) = op_table.insert_full(name.to_owned(), Rc::new(op_fn));
    assert!(prev.is_none());
    op_id
  }

  fn get_error_class_name(&self, err: &ErrBox) -> &'static str {
    get_error_class_name(err)
  }
}
