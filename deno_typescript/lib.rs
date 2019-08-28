extern crate deno;
extern crate serde;
extern crate serde_json;

mod ops;
use deno::deno_mod;
use deno::js_check;
pub use deno::v8_set_flags;
use deno::ErrBox;
use deno::Isolate;
use deno::ModuleSpecifier;
use deno::StartupData;
pub use ops::EmitResult;
use ops::WrittenFile;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

/// Sets the --trace-serializer V8 flag for debugging snapshots.
pub fn trace_serializer() {
  let dummy = "foo".to_string();
  let r =
    deno::v8_set_flags(vec![dummy.clone(), "--trace-serializer".to_string()]);
  assert_eq!(r, vec![dummy]);
}

#[derive(Debug)]
pub struct TSState {
  bundle: bool,
  exit_code: i32,

  emit_result: Option<EmitResult>,
  // (url, corresponding_module, source_code)
  written_files: Vec<WrittenFile>,
}

pub struct TSIsolate {
  isolate: Isolate,
  state: Arc<Mutex<TSState>>,
}

static TYPESCRIPT_CODE: &str =
  include_str!("../third_party/node_modules/typescript/lib/typescript.js");

impl TSIsolate {
  fn new(bundle: bool) -> TSIsolate {
    let mut isolate = Isolate::new(StartupData::None, false);
    let main_code = include_str!("compiler_main.js");
    js_check(isolate.execute("assets/typescript.js", TYPESCRIPT_CODE));
    js_check(isolate.execute("compiler_main.js", main_code));

    let state = Arc::new(Mutex::new(TSState {
      bundle,
      exit_code: 0,
      emit_result: None,
      written_files: Vec::new(),
    }));
    let state_ = state.clone();
    isolate.set_dispatch(move |op_id, control_buf, zero_copy_buf| {
      assert!(zero_copy_buf.is_none()); // zero_copy_buf unused in compiler.
      let mut s = state_.lock().unwrap();
      ops::dispatch_op(&mut s, op_id, control_buf)
    });
    TSIsolate { isolate, state }
  }

  // TODO(ry) Instead of Result<Arc<Mutex<TSState>>, ErrBox>, return something
  // like Result<TSState, ErrBox>. I think it would be nicer if this function
  // consumes TSIsolate.
  /// Compiles each module to ESM. Doesn't write any files to disk.
  /// Passes all output via state.
  fn compile(
    mut self,
    config_json: &serde_json::Value,
    root_names: Vec<String>,
  ) -> Result<Arc<Mutex<TSState>>, ErrBox> {
    let root_names_json = serde_json::json!(root_names).to_string();
    let source =
      &format!("main({:?}, {})", config_json.to_string(), root_names_json);
    self.isolate.execute("<anon>", source)?;
    Ok(self.state.clone())
  }
}

// TODO(ry) Instead of Result<Arc<Mutex<TSState>>, ErrBox>, return something
// like Result<TSState, ErrBox>
pub fn compile(
  root_names: Vec<PathBuf>,
) -> Result<Arc<Mutex<TSState>>, ErrBox> {
  let ts_isolate = TSIsolate::new(false);

  let config_json = serde_json::json!({
    "compilerOptions": {
      "declaration": true,
      "lib": ["esnext"],
      "module": "esnext",
      "target": "esnext",
      "listFiles": true,
      "listEmittedFiles": true,
      // Emit the source alongside the sourcemaps within a single file;
      // requires --inlineSourceMap or --sourceMap to be set.
      // "inlineSources": true,
      "sourceMap": true,
    },
  });

  let mut root_names_str: Vec<String> = root_names
    .iter()
    .map(|p| {
      assert!(p.exists());
      p.to_string_lossy().to_string()
    })
    .collect();
  root_names_str.push("$asset$/lib.deno_core.d.ts".to_string());

  // TODO lift js_check to caller?
  let state = js_check(ts_isolate.compile(&config_json, root_names_str));

  Ok(state)
}

pub fn compile_bundle(
  bundle: &Path,
  root_names: Vec<PathBuf>,
) -> Result<Arc<Mutex<TSState>>, ErrBox> {
  let ts_isolate = TSIsolate::new(true);

  let config_json = serde_json::json!({
    "compilerOptions": {
      "declaration": true,
      "lib": ["esnext"],
      "module": "amd",
      "target": "esnext",
      "listFiles": true,
      "listEmittedFiles": true,
      // "types" : ["typescript.d.ts"],
      "typeRoots" : ["$typeRoots$"],
      // Emit the source alongside the sourcemaps within a single file;
      // requires --inlineSourceMap or --sourceMap to be set.
      // "inlineSources": true,
      "sourceMap": true,
      "outFile": bundle,
    },
  });

  let mut root_names_str: Vec<String> = root_names
    .iter()
    .map(|p| {
      if !p.exists() {
        panic!("File not found {}", p.display());
      }

      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
      module_specifier.as_str().to_string()
      // p.to_string_lossy().to_string()
    })
    .collect();
  root_names_str.push("$asset$/lib.deno_core.d.ts".to_string());

  // TODO lift js_check to caller?
  let state = js_check(ts_isolate.compile(&config_json, root_names_str));

  Ok(state)
}

#[allow(dead_code)]
fn print_source_code(code: &str) {
  let mut i = 1;
  for line in code.lines() {
    println!("{:3}  {}", i, line);
    i += 1;
  }
}

pub fn mksnapshot_bundle(
  bundle: &Path,
  env_var: &str,
  state: Arc<Mutex<TSState>>,
) -> Result<(), ErrBox> {
  let mut runtime_isolate = Isolate::new(StartupData::None, true);
  let source_code_vec = std::fs::read(bundle)?;
  let source_code = std::str::from_utf8(&source_code_vec)?;

  js_check(
    runtime_isolate.execute("amd_runtime.js", include_str!("amd_runtime.js")),
  );

  js_check(runtime_isolate.execute("bundle", &source_code));

  // execute main.

  let state = state.lock().unwrap();
  let main = state.written_files.last().unwrap().module_name.clone();
  // let main = "file:///Users/rld/src/deno_typescript/cli_snapshots/js/main.ts";
  js_check(runtime_isolate.execute("anon", &format!("require('{}')", main)));

  snapshot_to_env(runtime_isolate, env_var)?;

  Ok(())
}

pub fn mksnapshot_bundle_ts(
  bundle: &Path,
  env_var: &str,
  state: Arc<Mutex<TSState>>,
) -> Result<(), ErrBox> {
  let mut runtime_isolate = Isolate::new(StartupData::None, true);
  let source_code_vec = std::fs::read(bundle)?;
  let source_code = std::str::from_utf8(&source_code_vec)?;

  js_check(
    runtime_isolate.execute("amd_runtime.js", include_str!("amd_runtime.js")),
  );
  js_check(runtime_isolate.execute(&bundle.to_string_lossy(), &source_code));

  let state = state.lock().unwrap();
  let main = state.written_files.last().unwrap().module_name.clone();

  js_check(runtime_isolate.execute("typescript.js", TYPESCRIPT_CODE));
  js_check(runtime_isolate.execute("anon", &format!("require('{}')", main)));

  snapshot_to_env(runtime_isolate, env_var)?;

  Ok(())
}

// TODO(ry) Instead of state: Arc<Mutex<TSState>>, take something like state:
// &TSState
pub fn mksnapshot(
  env_var: &str,
  state: Arc<Mutex<TSState>>,
) -> Result<(), ErrBox> {
  assert!(!state.lock().unwrap().bundle);

  let mut runtime_isolate = Isolate::new(StartupData::None, true);
  let mut url2id: HashMap<String, deno_mod> = HashMap::new();
  let mut id2url: HashMap<deno_mod, String> = HashMap::new();

  let state = state.lock().unwrap();

  let main = state.written_files.last().unwrap().module_name.clone();

  for f in state.written_files.iter() {
    if f.url.ends_with(".js") {
      let is_main = f.module_name == main;
      let id =
        js_check(runtime_isolate.mod_new(is_main, &f.url, &f.source_code));
      url2id.insert(f.module_name.clone(), id);
      id2url.insert(id, f.module_name.clone());

      if f.url.ends_with("flatbuffers/flatbuffers.js") {
        print_source_code(&f.source_code);
      }
    }
  }

  let url2id_ = url2id.clone(); // FIXME
  let mut resolve = move |specifier: &str, referrer: deno_mod| -> deno_mod {
    let referrer_url = id2url.get(&referrer).unwrap();
    let import_url =
      ModuleSpecifier::resolve_import(specifier, referrer_url.as_str())
        .unwrap();
    let import_url_str = import_url.as_str();
    *url2id_.get(import_url_str).unwrap_or_else(|| {
      panic!("Could not resolve {}", import_url_str);
    })
  };

  // Instantiate each module.
  for (_url, id) in url2id.iter() {
    js_check(runtime_isolate.mod_instantiate(*id, &mut resolve));
  }

  // Execute the main module.
  let main_id = url2id.get(main.as_str()).unwrap();
  js_check(runtime_isolate.mod_evaluate(*main_id));

  snapshot_to_env(runtime_isolate, env_var)?;
  Ok(())
}

fn snapshot_to_env(
  runtime_isolate: Isolate,
  env_var: &str,
) -> Result<(), ErrBox> {
  println!("creating snapshot...");
  let snapshot = runtime_isolate.snapshot()?;
  let snapshot_slice =
    unsafe { std::slice::from_raw_parts(snapshot.data_ptr, snapshot.data_len) };
  println!("snapshot bytes {}", snapshot_slice.len());
  //
  let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
  let snapshot_path = out_dir.join(env_var);

  fs::write(&snapshot_path, snapshot_slice)?;
  println!("snapshot path {} ", snapshot_path.display());
  println!("cargo:rustc-env={}={}", env_var, snapshot_path.display());
  Ok(())
}

macro_rules! inc {
  ($e:expr) => {
    include_str!(concat!("../third_party/node_modules/typescript/lib/", $e))
  };
}

pub fn get_asset(asset: &str) -> String {
  String::from(match asset {
    "lib.deno_core.d.ts" => include_str!("lib.deno_core.d.ts"),
    "lib.esnext.d.ts" => inc!("lib.esnext.d.ts"),
    "lib.es2019.d.ts" => inc!("lib.es2019.d.ts"),
    "lib.es2018.d.ts" => inc!("lib.es2018.d.ts"),
    "lib.es2017.d.ts" => inc!("lib.es2017.d.ts"),
    "lib.es2016.d.ts" => inc!("lib.es2016.d.ts"),
    "lib.es5.d.ts" => inc!("lib.es5.d.ts"),
    "lib.es2015.d.ts" => inc!("lib.es2015.d.ts"),
    "lib.es2015.core.d.ts" => inc!("lib.es2015.core.d.ts"),
    "lib.es2015.collection.d.ts" => inc!("lib.es2015.collection.d.ts"),
    "lib.es2015.generator.d.ts" => inc!("lib.es2015.generator.d.ts"),
    "lib.es2015.iterable.d.ts" => inc!("lib.es2015.iterable.d.ts"),
    "lib.es2015.promise.d.ts" => inc!("lib.es2015.promise.d.ts"),
    "lib.es2015.symbol.d.ts" => inc!("lib.es2015.symbol.d.ts"),
    "lib.es2015.proxy.d.ts" => inc!("lib.es2015.proxy.d.ts"),
    "lib.es2015.symbol.wellknown.d.ts" => {
      inc!("lib.es2015.symbol.wellknown.d.ts")
    }
    "lib.es2015.reflect.d.ts" => inc!("lib.es2015.reflect.d.ts"),
    "lib.es2016.array.include.d.ts" => inc!("lib.es2016.array.include.d.ts"),
    "lib.es2017.object.d.ts" => inc!("lib.es2017.object.d.ts"),
    "lib.es2017.sharedmemory.d.ts" => inc!("lib.es2017.sharedmemory.d.ts"),
    "lib.es2017.string.d.ts" => inc!("lib.es2017.string.d.ts"),
    "lib.es2017.intl.d.ts" => inc!("lib.es2017.intl.d.ts"),
    "lib.es2017.typedarrays.d.ts" => inc!("lib.es2017.typedarrays.d.ts"),
    "lib.es2018.asynciterable.d.ts" => inc!("lib.es2018.asynciterable.d.ts"),
    "lib.es2018.promise.d.ts" => inc!("lib.es2018.promise.d.ts"),
    "lib.es2018.regexp.d.ts" => inc!("lib.es2018.regexp.d.ts"),
    "lib.es2018.intl.d.ts" => inc!("lib.es2018.intl.d.ts"),
    "lib.es2019.array.d.ts" => inc!("lib.es2019.array.d.ts"),
    "lib.es2019.object.d.ts" => inc!("lib.es2019.object.d.ts"),
    "lib.es2019.string.d.ts" => inc!("lib.es2019.string.d.ts"),
    "lib.es2019.symbol.d.ts" => inc!("lib.es2019.symbol.d.ts"),
    "lib.esnext.bigint.d.ts" => inc!("lib.esnext.bigint.d.ts"),
    "lib.esnext.intl.d.ts" => inc!("lib.esnext.intl.d.ts"),
    _ => panic!("Unknown asset {}", asset),
  })
}
