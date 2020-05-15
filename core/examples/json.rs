#[macro_use]
extern crate derive_deref;
#[macro_use]
extern crate log;

use deno_core::*;
use futures::future::FutureExt;
use futures::task::Context;
use futures::task::Poll;
use serde_json::json;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

struct Logger;

impl log::Log for Logger {
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    metadata.level() <= log::max_level()
  }

  fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
      println!("{} - {}", record.level(), record.args());
    }
  }

  fn flush(&self) {}
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum OpErrorKind {
  JsonError = 1,
  BadResourceId = 2,
}

#[derive(Debug)]
struct OpError {
  kind: OpErrorKind,
  msg: String,
}

impl OpError {
  fn bad_rid() -> Self {
    Self {
      kind: OpErrorKind::BadResourceId,
      msg: "Bad resource id".to_string(),
    }
  }
}

impl JsonError for OpError {
  fn kind(&self) -> JsonErrorKind {
    (self.kind as u32).into()
  }

  fn msg(&self) -> String {
    self.msg.clone()
  }
}

impl From<serde_json::Error> for OpError {
  fn from(e: serde_json::Error) -> Self {
    Self {
      kind: OpErrorKind::JsonError,
      msg: e.to_string(),
    }
  }
}

struct StaticLoader {
  pub modules: HashMap<String, String>,
}

impl ModuleLoader for StaticLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, ErrBox> {
    let module_specifier =
      ModuleSpecifier::resolve_import(specifier, referrer)?;

    Ok(module_specifier)
  }

  /// Given an absolute url, load its source code.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    _is_dyn_import: bool,
  ) -> Pin<Box<ModuleSourceFuture>> {
    debug!(
      "module load specifier={} referrer={:?}",
      module_specifier, maybe_referrer
    );
    let module_specifier = module_specifier.clone();

    let module_url_specified = module_specifier.to_string();
    let module_url_found = module_url_specified.clone();
    let code = self.modules.get(&module_url_specified).unwrap().clone();
    let fut = async move {
      Ok(ModuleSource {
        code,
        module_url_specified,
        module_url_found,
      })
    };

    fut.boxed_local()
  }
}

struct Isolate {
  isolate: Box<EsIsolate>, // Unclear why CoreIsolate::new() returns a box.
  state: State,
}

#[derive(Clone, Default, Deref)]
struct State(Rc<RefCell<StateInner>>);

#[derive(Default)]
struct StateInner {
  resource_table: ResourceTable,
}

impl Isolate {
  pub fn new() -> Self {
    let modules = vec![
      (
        "file:///examples/json.js".to_string(),
        include_str!("./json.js").to_string(),
      ),
      (
        "file:///dispatch_json.js".to_string(),
        include_str!("./../dispatch_json.js").to_string(),
      ),
    ];
    let loader = StaticLoader {
      modules: modules.into_iter().collect(),
    };

    let startup_data = StartupData::None;

    let mut isolate = Self {
      isolate: EsIsolate::new(Rc::new(loader), startup_data, false),
      state: Default::default(),
    };

    isolate.register_op("new_counter", op_new_counter);
    isolate.register_op("count", op_count);

    isolate
  }

  fn register_op<D>(&mut self, name: &'static str, dispatch: D)
  where
    D: Fn(
        &mut CoreIsolate,
        &State,
        Value,
        Option<ZeroCopyBuf>,
      ) -> Result<JsonOp<OpError>, OpError>
      + Copy
      + 'static,
  {
    let state = self.state.clone();
    let stateful_op = move |isolate: &mut CoreIsolate,
                            args: Value,
                            zero_copy: Option<ZeroCopyBuf>|
          -> Result<JsonOp<OpError>, OpError> {
      dispatch(isolate, &state, args, zero_copy)
    };
    let core_op = json_op(stateful_op);

    self.isolate.register_op(name, core_op);
  }

  pub async fn execute(&mut self) -> Result<(), ErrBox> {
    let main_module =
      ModuleSpecifier::resolve_url_or_path("file:///examples/json.js").unwrap();
    let id = self.isolate.load_module(&main_module, None).await?;
    self.isolate.mod_evaluate(id)
  }
}

impl Future for Isolate {
  type Output = <EsIsolate as Future>::Output;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    self.isolate.poll_unpin(cx)
  }
}

struct Counter(u32);

#[derive(Deserialize)]
struct OpNewCounterArgs {
  pub start: u32,
}

fn op_new_counter(
  _isolate: &mut CoreIsolate,
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp<OpError>, OpError> {
  let args: OpNewCounterArgs = serde_json::from_value(args)?;
  debug!("new counter start={}", args.start);

  let counter = Counter(args.start);
  let resource_table = &mut state.borrow_mut().resource_table;
  let rid = resource_table.add("counter", Box::new(counter));

  Ok(JsonOp::Sync(json!({ "rid": rid })))
}

#[derive(Deserialize)]
struct OpCountArgs {
  pub rid: u32,
  pub step: u32,
}

fn op_count(
  _isolate: &mut CoreIsolate,
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp<OpError>, OpError> {
  let args: OpCountArgs = serde_json::from_value(args)?;
  debug!("count rid={} step={}", args.rid, args.step);

  let resource_table = &mut state.borrow_mut().resource_table;
  let mut counter: &mut Counter = match resource_table.get_mut(0) {
    Some(c) => c,
    None => return Err(OpError::bad_rid()),
  };
  counter.0 += args.step;

  Ok(JsonOp::Sync(json!({ "count": counter.0 })))
}

fn main() {
  log::set_logger(&Logger).unwrap();
  log::set_max_level(
    env::args()
      .find(|a| a == "-D")
      .map(|_| log::LevelFilter::Debug)
      .unwrap_or(log::LevelFilter::Warn),
  );

  // NOTE: `--help` arg will display V8 help and exit
  deno_core::v8_set_flags(env::args().collect());

  let mut runtime = tokio::runtime::Builder::new()
    .basic_scheduler()
    .enable_all()
    .build()
    .unwrap();
  let result = runtime.block_on(execute());
  if let Err(err) = result {
    eprintln!("{}", err.to_string());
    std::process::exit(1);
  }
}

async fn execute() -> Result<(), ErrBox> {
  let mut isolate = Isolate::new();
  (&mut isolate).await?;
  isolate.execute().await?;
  (&mut isolate).await?;
  Ok(())
}
