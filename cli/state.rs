// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::file_fetcher::SourceFileFetcher;
use crate::global_state::GlobalState;
use crate::global_timer::GlobalTimer;
use crate::import_map::ImportMap;
use crate::metrics::Metrics;
use crate::op_error::OpError;
use crate::ops::JsonOp;
use crate::ops::MinimalOp;
use crate::permissions::Permissions;
use crate::tsc::TargetLib;
use crate::web_worker::WebWorkerHandle;
use deno_core::Buf;
use deno_core::ErrBox;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::Op;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use futures::Future;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::thread::JoinHandle;
use std::time::Instant;

#[derive(Clone)]
pub struct State(Rc<RefCell<StateInner>>);

impl Deref for State {
  type Target = Rc<RefCell<StateInner>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct StateInner {
  pub global_state: GlobalState,
  pub permissions: Permissions,
  pub main_module: ModuleSpecifier,
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub metrics: Metrics,
  pub global_timer: GlobalTimer,
  pub workers: HashMap<u32, (JoinHandle<()>, WebWorkerHandle)>,
  pub next_worker_id: u32,
  pub start_time: Instant,
  pub seeded_rng: Option<StdRng>,
  pub target_lib: TargetLib,
  pub is_main: bool,
  pub is_internal: bool,
}

impl State {
  pub fn stateful_json_op<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(&mut deno_core::CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op
  where
    D: Fn(&State, Value, &mut [ZeroCopyBuf]) -> Result<JsonOp, OpError>,
  {
    use crate::ops::json_op;
    self.core_op(json_op(self.stateful_op(dispatcher)))
  }

  pub fn stateful_json_op2<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(&mut deno_core::CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op
  where
    D: Fn(
      &mut deno_core::CoreIsolateState,
      &State,
      Value,
      &mut [ZeroCopyBuf],
    ) -> Result<JsonOp, OpError>,
  {
    use crate::ops::json_op;
    self.core_op(json_op(self.stateful_op2(dispatcher)))
  }

  /// Wrap core `OpDispatcher` to collect metrics.
  // TODO(ry) this should be private. Is called by stateful_json_op or
  // stateful_minimal_op
  pub fn core_op<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(&mut deno_core::CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op
  where
    D: Fn(&mut deno_core::CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op,
  {
    let state = self.clone();

    move |isolate_state: &mut deno_core::CoreIsolateState,
          control: &[u8],
          zero_copy: &mut [ZeroCopyBuf]|
          -> Op {
      let bytes_sent_control = control.len() as u64;
      let bytes_sent_zero_copy =
        zero_copy.iter().map(|b| b.len()).sum::<usize>() as u64;

      let op = dispatcher(isolate_state, control, zero_copy);

      match op {
        Op::Sync(buf) => {
          let mut state_ = state.borrow_mut();
          state_.metrics.op_sync(
            bytes_sent_control,
            bytes_sent_zero_copy,
            buf.len() as u64,
          );
          Op::Sync(buf)
        }
        Op::Async(fut) => {
          let mut state_ = state.borrow_mut();
          state_
            .metrics
            .op_dispatched_async(bytes_sent_control, bytes_sent_zero_copy);
          let state = state.clone();
          let result_fut = fut.map(move |buf: Buf| {
            let mut state_ = state.borrow_mut();
            state_.metrics.op_completed_async(buf.len() as u64);
            buf
          });
          Op::Async(result_fut.boxed_local())
        }
        Op::AsyncUnref(fut) => {
          let mut state_ = state.borrow_mut();
          state_.metrics.op_dispatched_async_unref(
            bytes_sent_control,
            bytes_sent_zero_copy,
          );
          let state = state.clone();
          let result_fut = fut.map(move |buf: Buf| {
            let mut state_ = state.borrow_mut();
            state_.metrics.op_completed_async_unref(buf.len() as u64);
            buf
          });
          Op::AsyncUnref(result_fut.boxed_local())
        }
      }
    }
  }

  pub fn stateful_minimal_op2<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(&mut deno_core::CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op
  where
    D: Fn(
      &mut deno_core::CoreIsolateState,
      &State,
      bool,
      i32,
      &mut [ZeroCopyBuf],
    ) -> MinimalOp,
  {
    let state = self.clone();
    self.core_op(crate::ops::minimal_op(
      move |isolate_state: &mut deno_core::CoreIsolateState,
            is_sync: bool,
            rid: i32,
            zero_copy: &mut [ZeroCopyBuf]|
            -> MinimalOp {
        dispatcher(isolate_state, &state, is_sync, rid, zero_copy)
      },
    ))
  }

  /// This is a special function that provides `state` argument to dispatcher.
  ///
  /// NOTE: This only works with JSON dispatcher.
  /// This is a band-aid for transition to `CoreIsolate.register_op` API as most of our
  /// ops require `state` argument.
  pub fn stateful_op<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(
    &mut deno_core::CoreIsolateState,
    Value,
    &mut [ZeroCopyBuf],
  ) -> Result<JsonOp, OpError>
  where
    D: Fn(&State, Value, &mut [ZeroCopyBuf]) -> Result<JsonOp, OpError>,
  {
    let state = self.clone();
    move |_isolate_state: &mut deno_core::CoreIsolateState,
          args: Value,
          zero_copy: &mut [ZeroCopyBuf]|
          -> Result<JsonOp, OpError> { dispatcher(&state, args, zero_copy) }
  }

  pub fn stateful_op2<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(
    &mut deno_core::CoreIsolateState,
    Value,
    &mut [ZeroCopyBuf],
  ) -> Result<JsonOp, OpError>
  where
    D: Fn(
      &mut deno_core::CoreIsolateState,
      &State,
      Value,
      &mut [ZeroCopyBuf],
    ) -> Result<JsonOp, OpError>,
  {
    let state = self.clone();
    move |isolate_state: &mut deno_core::CoreIsolateState,
          args: Value,
          zero_copy: &mut [ZeroCopyBuf]|
          -> Result<JsonOp, OpError> {
      dispatcher(isolate_state, &state, args, zero_copy)
    }
  }

  /// Quits the process if the --unstable flag was not provided.
  ///
  /// This is intentionally a non-recoverable check so that people cannot probe
  /// for unstable APIs from stable programs.
  pub fn check_unstable(&self, api_name: &str) {
    // TODO(ry) Maybe use IsolateHandle::terminate_execution here to provide a
    // stack trace in JS.
    let s = self.0.borrow();
    if !s.global_state.flags.unstable {
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
      if let Some(import_map) = &self.borrow().import_map {
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
    let mut state = self.borrow_mut();
    // TODO(bartlomieju): incrementing resolve_count here has no sense...
    state.metrics.resolve_count += 1;
    let module_url_specified = module_specifier.to_string();
    let global_state = state.global_state.clone();

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
    let state = self.borrow();
    let target_lib = state.target_lib.clone();
    let maybe_import_map = state.import_map.clone();
    // Only "main" module is loaded without permission check,
    // ie. module that is associated with "is_main" state
    // and is not a dynamic import.
    let permissions = if state.is_main && !is_dyn_import {
      Permissions::allow_all()
    } else {
      state.permissions.clone()
    };
    let global_state = state.global_state.clone();
    // TODO(bartlomieju): I'm not sure if it's correct to ignore
    // bad referrer - this is the case for `Deno.core.evalContext()` where
    // `ref_str` is `<unknown>`.
    let maybe_referrer = if let Some(ref_str) = maybe_referrer {
      ModuleSpecifier::resolve_url(&ref_str).ok()
    } else {
      None
    };
    drop(state);

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
    global_state: GlobalState,
    shared_permissions: Option<Permissions>,
    main_module: ModuleSpecifier,
    maybe_import_map: Option<ImportMap>,
    is_internal: bool,
  ) -> Result<Self, ErrBox> {
    let seeded_rng = match global_state.flags.seed {
      Some(seed) => Some(StdRng::seed_from_u64(seed)),
      None => None,
    };

    let permissions = if let Some(perm) = shared_permissions {
      perm
    } else {
      global_state.permissions.clone()
    };

    let state = Rc::new(RefCell::new(StateInner {
      global_state,
      main_module,
      permissions,
      import_map: maybe_import_map,
      metrics: Metrics::default(),
      global_timer: GlobalTimer::new(),
      workers: HashMap::new(),
      next_worker_id: 0,
      start_time: Instant::now(),
      seeded_rng,
      target_lib: TargetLib::Main,
      is_main: true,
      is_internal,
    }));

    Ok(Self(state))
  }

  /// If `shared_permission` is None then permissions from globa state are used.
  pub fn new_for_worker(
    global_state: GlobalState,
    shared_permissions: Option<Permissions>,
    main_module: ModuleSpecifier,
  ) -> Result<Self, ErrBox> {
    let seeded_rng = match global_state.flags.seed {
      Some(seed) => Some(StdRng::seed_from_u64(seed)),
      None => None,
    };

    let permissions = if let Some(perm) = shared_permissions {
      perm
    } else {
      global_state.permissions.clone()
    };

    let state = Rc::new(RefCell::new(StateInner {
      global_state,
      main_module,
      permissions,
      import_map: None,
      metrics: Metrics::default(),
      global_timer: GlobalTimer::new(),
      workers: HashMap::new(),
      next_worker_id: 0,
      start_time: Instant::now(),
      seeded_rng,
      target_lib: TargetLib::Worker,
      is_main: false,
      is_internal: false,
    }));

    Ok(Self(state))
  }

  #[inline]
  pub fn check_read(&self, path: &Path) -> Result<(), OpError> {
    self.borrow().permissions.check_read(path)
  }

  /// As `check_read()`, but permission error messages will anonymize the path
  /// by replacing it with the given `display`.
  #[inline]
  pub fn check_read_blind(
    &self,
    path: &Path,
    display: &str,
  ) -> Result<(), OpError> {
    self.borrow().permissions.check_read_blind(path, display)
  }

  #[inline]
  pub fn check_write(&self, path: &Path) -> Result<(), OpError> {
    self.borrow().permissions.check_write(path)
  }

  #[inline]
  pub fn check_env(&self) -> Result<(), OpError> {
    self.borrow().permissions.check_env()
  }

  #[inline]
  pub fn check_net(&self, hostname: &str, port: u16) -> Result<(), OpError> {
    self.borrow().permissions.check_net(hostname, port)
  }

  #[inline]
  pub fn check_net_url(&self, url: &url::Url) -> Result<(), OpError> {
    self.borrow().permissions.check_net_url(url)
  }

  #[inline]
  pub fn check_run(&self) -> Result<(), OpError> {
    self.borrow().permissions.check_run()
  }

  #[inline]
  pub fn check_plugin(&self, filename: &Path) -> Result<(), OpError> {
    self.borrow().permissions.check_plugin(filename)
  }

  pub fn check_dyn_import(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), OpError> {
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
  pub fn mock(main_module: &str) -> State {
    let module_specifier = ModuleSpecifier::resolve_url_or_path(main_module)
      .expect("Invalid entry module");
    State::new(
      GlobalState::mock(vec!["deno".to_string()]),
      None,
      module_specifier,
      None,
      false,
    )
    .unwrap()
  }
}
