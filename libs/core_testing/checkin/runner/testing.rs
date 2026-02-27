// Copyright 2018-2025 the Deno authors. MIT license.

use anyhow::bail;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::PollEventLoopOptions;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use super::Output;
use super::TestData;
use super::create_runtime_from_snapshot;
use super::run_async;
use super::snapshot;

deno_core::extension!(
  checkin_testing,
  ops = [
    op_test_register,
  ],
  esm_entry_point = "checkin:testing",
  esm = [
    dir "checkin/runtime",
    "checkin:testing" = "testing.ts",
  ],
  state = |state| {
    state.put(TestFunctions::default());
  }
);

#[derive(Default)]
pub struct TestFunctions {
  pub functions: Vec<(String, v8::Global<v8::Function>)>,
}

#[op2]
pub fn op_test_register(
  op_state: &mut OpState,
  #[string] name: String,
  #[scoped] f: v8::Global<v8::Function>,
) {
  op_state
    .borrow_mut::<TestFunctions>()
    .functions
    .push((name, f));
}

fn create_runtime() -> JsRuntime {
  static SNAPSHOT: OnceLock<Box<[u8]>> = OnceLock::new();

  let snapshot = SNAPSHOT.get_or_init(snapshot::create_snapshot);

  create_runtime_from_snapshot(
    snapshot,
    false,
    None,
    vec![checkin_testing::init()],
  )
  .0
}

/// Run a integration test within the `checkin` runtime. This executes a single file, imports and all,
/// and compares its output with the `.out` file in the same directory.
pub fn run_integration_test(test: &str) {
  let runtime = create_runtime();
  run_async(run_integration_test_task(runtime, test.to_owned()));
}

async fn run_integration_test_task(
  mut runtime: JsRuntime,
  test: String,
) -> Result<(), anyhow::Error> {
  let test_dir = get_test_dir(&["integration", &test]);
  let url = get_test_url(&test_dir, &test)?;
  runtime
    .op_state()
    .borrow_mut()
    .put(deno_core::error::InitialCwd(Arc::new(Url::parse(
      "test:///",
    )?)));
  let maybe_error = match runtime.load_main_es_module(&url).await {
    Err(err) => Some(err),
    Ok(module) => {
      let f = runtime.mod_evaluate(module);
      match runtime
        .run_event_loop(PollEventLoopOptions::default())
        .await
      {
        Err(err) => Some(err),
        _ => f.await.err(),
      }
    }
  };
  if let Some(err) = maybe_error {
    let state = runtime.op_state().clone();
    let state = state.borrow();
    let output: &Output = state.borrow();
    for line in err.to_string().split('\n') {
      output.line(format!("[ERR] {line}"));
    }
  }
  let mut lines = runtime.op_state().borrow_mut().take::<Output>().take();
  lines.push(String::new());
  let mut expected_output = String::new();
  File::open(test_dir.join(format!("{test}.out")))
    .await?
    .read_to_string(&mut expected_output)
    .await?;
  let actual_output = lines.join("\n");
  assert_eq!(actual_output, expected_output);
  Ok(())
}

/// Run a unit test within the `checkin` runtime. This loads a file which registers a number of tests,
/// then each test is run individually and failures are printed.
pub fn run_unit_test(test: &str) {
  let runtime = create_runtime();
  run_async(run_unit_test_task(runtime, test.to_owned()));
}

async fn run_unit_test_task(
  mut runtime: JsRuntime,
  test: String,
) -> Result<(), anyhow::Error> {
  let test_dir = get_test_dir(&["unit"]);
  let url = get_test_url(&test_dir, &test)?;
  runtime
    .op_state()
    .borrow_mut()
    .put(deno_core::error::InitialCwd(Arc::new(Url::parse(
      "test:///",
    )?)));
  let module = runtime.load_main_es_module(&url).await?;
  let f = runtime.mod_evaluate(module);
  runtime
    .run_event_loop(PollEventLoopOptions::default())
    .await?;
  f.await?;

  let tests: TestFunctions = runtime.op_state().borrow_mut().take();
  for (name, function) in tests.functions {
    println!("Testing {name}...");
    let call = runtime.call(&function);
    runtime
      .with_event_loop_promise(call, PollEventLoopOptions::default())
      .await?;

    // Clear any remaining test data so we have a fresh state
    let state = runtime.op_state();
    let mut state = state.borrow_mut();
    let data = state.borrow_mut::<TestData>();
    data.data.clear();
  }

  Ok(())
}

fn get_test_dir(dirs: &[&str]) -> PathBuf {
  let mut test_dir = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
  for dir in dirs {
    test_dir.join(dir).clone_into(&mut test_dir);
  }

  test_dir.to_owned()
}

fn get_test_url(test_dir: &Path, test: &str) -> Result<Url, anyhow::Error> {
  let mut path = None;
  for extension in ["ts", "js", "nocompile"] {
    let test_path = test_dir.join(format!("{test}.{extension}"));
    if test_path.exists() {
      path = Some(test_path);
      break;
    }
  }
  let Some(path) = path else {
    bail!("Test file not found");
  };
  let path = path.canonicalize()?.to_owned();
  let url = Url::from_file_path(path).unwrap().to_string();
  let base_url = Url::from_file_path(Path::new(env!("CARGO_MANIFEST_DIR")))
    .unwrap()
    .to_string();
  let url = Url::parse(&format!("test://{}", &url[base_url.len()..]))?;
  Ok(url)
}
