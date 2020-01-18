// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// #![deny(warnings)]
#![allow(unused)]

extern crate deno_core;
extern crate serde;
extern crate serde_json;

use deno_core::js_check;
use deno_core::CoreOp;
use deno_core::ErrBox;
use deno_core::Isolate;
use deno_core::EsIsolate;
use deno_core::Loader;
use deno_core::ModuleSpecifier;
use deno_core::PinnedBuf;
use deno_core::SourceCodeInfoFuture;
use deno_core::SourceCodeInfo;
use futures;
use futures::future::FutureExt;
use std::pin::Pin;
use deno_core::StartupData;
use crate::new_ops;
use new_ops::EmitResult;
use new_ops::WrittenFile;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

static TYPESCRIPT_CODE: &str = include_str!("typescript/lib/typescript.js");
static COMPILER_CODE: &str = include_str!("compiler_main.js");
static BUNDLE_LOADER: &str = include_str!("bundle_loader.js");

pub fn ts_version() -> String {
  let data = include_str!("typescript/package.json");
  let pkg: serde_json::Value = serde_json::from_str(data).unwrap();
  pkg["version"].as_str().unwrap().to_string()
}

#[derive(Debug)]
pub struct NewTsState {
  pub bundle: bool,
  pub exit_code: i32,
  pub emit_result: Option<EmitResult>,
  /// A list of files emitted by typescript. WrittenFile is tuple of the form
  /// (url, corresponding_module, source_code)
  pub written_files: Vec<WrittenFile>,
}

impl NewTsState {
  fn main_module_name(&self) -> String {
    // Assuming that TypeScript has emitted the main file last.
    self.written_files.last().unwrap().module_name.clone()
  }

  fn main_module_url(&self) -> String {
    // Assuming that TypeScript has emitted the main file last.
    self.written_files.last().unwrap().url.clone()
  }

  fn get_compiled_file(&self, specifier: &str) -> Option<WrittenFile> {
    let mut path = PathBuf::from(specifier);

    if specifier.ends_with(".d.ts") {
      return Some(WrittenFile {
        url: specifier.to_string(),
        module_name: specifier.to_string(),
        source_code: "".to_string(),
      })
    }

    if specifier.ends_with(".ts") {
      path.set_extension("js");
    }

    let path_str = path.to_str().unwrap();
    // eprintln!("compiled file {}", path_str);

    for file in self.written_files.iter() {
      if path_str.contains(&file.url) {
        return Some(file.clone())
      }
    }

    None
  }
}

fn new_compiler_op<D>(
  ts_state: Arc<Mutex<NewTsState>>,
  dispatcher: D,
) -> impl Fn(&[u8], Option<PinnedBuf>) -> CoreOp
where
  D: Fn(&mut NewTsState, &[u8]) -> CoreOp,
{
  move |control: &[u8], zero_copy_buf: Option<PinnedBuf>| -> CoreOp {
    assert!(zero_copy_buf.is_none()); // zero_copy_buf unused in compiler.
    let mut s = ts_state.lock().unwrap();
    dispatcher(&mut s, control)
  }
}

pub struct TSIsolate {
  isolate: Box<Isolate>,
  state: Arc<Mutex<NewTsState>>,
}

impl TSIsolate {
  fn new(bundle: bool) -> TSIsolate {
    let mut isolate = Isolate::new(StartupData::None, false);
    js_check(isolate.execute("assets/typescript.js", TYPESCRIPT_CODE));
    js_check(isolate.execute("compiler_main.js", COMPILER_CODE));

    let state = Arc::new(Mutex::new(NewTsState {
      bundle,
      exit_code: 0,
      emit_result: None,
      written_files: Vec::new(),
    }));

    isolate.register_op(
      "readFile",
      new_compiler_op(state.clone(), new_ops::json_op(new_ops::read_file)),
    );
    isolate
      .register_op("exit", new_compiler_op(state.clone(), new_ops::json_op(new_ops::exit)));
    isolate.register_op(
      "writeFile",
      new_compiler_op(state.clone(), new_ops::json_op(new_ops::write_file)),
    );
    isolate.register_op(
      "resolveModuleNames",
      new_compiler_op(state.clone(), new_ops::json_op(new_ops::resolve_module_names)),
    );
    isolate.register_op(
      "setEmitResult",
      new_compiler_op(state.clone(), new_ops::json_op(new_ops::set_emit_result)),
    );

    TSIsolate { isolate, state }
  }

  // TODO(ry) Instead of Result<Arc<Mutex<NewTsState>>, ErrBox>, return something
  // like Result<NewTsState, ErrBox>. I think it would be nicer if this function
  // consumes TSIsolate.
  /// Compiles each module to ESM. Doesn't write any files to disk.
  /// Passes all output via state.
  fn compile(
    mut self,
    config_json: &serde_json::Value,
    root_names: Vec<String>,
  ) -> Result<Arc<Mutex<NewTsState>>, ErrBox> {
    let root_names_json = serde_json::json!(root_names).to_string();
    let source =
      &format!("main({:?}, {})", config_json.to_string(), root_names_json);
    self.isolate.execute("<anon>", source)?;
    Ok(self.state)
  }
}

#[allow(dead_code)]
fn print_source_code(code: &str) {
  let mut i = 1;
  for line in code.lines() {
    println!("{:3}  {}", i, line);
    i += 1;
  }
}
#[derive(Clone)]
struct TempLoader {
  ts_state: Arc<Mutex<NewTsState>>,
}

impl Loader for TempLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
    _is_dyn_import: bool,
  ) -> Result<ModuleSpecifier, ErrBox> {
    ModuleSpecifier::resolve_import(specifier, referrer).map_err(ErrBox::from)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Pin<Box<SourceCodeInfoFuture>> {
    let specifier_str = module_specifier.to_string();
    let state = self.ts_state.lock().unwrap();
    let file = state.get_compiled_file(&specifier_str).expect("Compiled file not found");
    let info =  SourceCodeInfo {
      code: file.source_code.to_string(),
      module_url_specified: module_specifier.to_string(),
      module_url_found: module_specifier.to_string(),
    };

    // if specifier_str.ends_with("compiler.ts") || specifier_str.ends_with("globals.ts")|| specifier_str.ends_with("dispatch.ts") {
    //   eprintln!("code: {}", info.code);  
    // }

    return futures::future::ok(info).boxed();
  }
}

pub fn create_ts_snapshot(
  _current_dir: &Path,
  root_names: Vec<PathBuf>,
  output_path: &Path,
) -> Result<(), ErrBox> {
  let mut root_names_str: Vec<String> = root_names
    .iter()
    .map(|p| {
      if !p.exists() {
        panic!("File not found {}", p.display());
      }

      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
      module_specifier.as_str().to_string()
    })
    .collect();

  let ts_isolate = TSIsolate::new(false);

  // TODO:
  let config_json = serde_json::json!({
    "compilerOptions": {
      "esModuleInterop": true,
      // "strict": true,
      // "declaration": true,
      "lib": ["esnext"],
      "module": "es2015",
      "target": "esnext",
      "listFiles": true,
      "listEmittedFiles": true,
      // "types" : ["typescript.d.ts"],
      "typeRoots" : ["$typeRoots$"],
      // Emit the source alongside the sourcemaps within a single file;
      // requires --inlineSourceMap or --sourceMap to be set.
      // "inlineSources": true,
      "inlineSourceMap": true,
      "inlineSources": true,
      // "sourceMap": true,
      "outDir": "deno:///",
    },
  });
  root_names_str.push("$asset$/lib.deno_core.d.ts".to_string());
  let state = ts_isolate.compile(&config_json, root_names_str)?;
  let temp_loader = Box::new(TempLoader { ts_state: state.clone() });
  let runtime_isolate = &mut EsIsolate::new(temp_loader.clone(), StartupData::None, true);
  runtime_isolate.register_op(
    "fetch_asset",
    new_compiler_op(state.clone(), new_ops::json_op(new_ops::fetch_asset)),
  );
  js_check(runtime_isolate.execute("typescript.js", TYPESCRIPT_CODE));
  let main_module = "deno:///compiler.ts";
  eprintln!("main module {}", main_module);
  let id = futures::executor::block_on(runtime_isolate.load_module(&main_module, None))?;
  eprintln!("module loaded {}", id);
  let result = runtime_isolate.mod_evaluate(id);
  eprintln!("module eval {:?}", result);
  result?;
  let isolate_future = runtime_isolate.boxed();
  futures::executor::block_on(isolate_future)?;

  runtime_isolate.clear_module_handles();

  
  eprintln!("post modules clear");
  println!("creating snapshot...");
  let snapshot = runtime_isolate.snapshot()?;
  let snapshot_slice: &[u8] = &*snapshot;
  println!("snapshot bytes {}", snapshot_slice.len());

  let snapshot_path = output_path.with_extension("bin");

  fs::write(&snapshot_path, snapshot_slice)?;
  println!("snapshot path {} ", snapshot_path.display());

  Ok(())
}

/// Same as get_asset() but returns NotFound intead of None.
pub fn get_asset2(name: &str) -> Result<String, ErrBox> {
  match get_asset(name) {
    Some(a) => Ok(a),
    None => Err(
      std::io::Error::new(std::io::ErrorKind::NotFound, "Asset not found")
        .into(),
    ),
  }
}

fn read_file(name: &str) -> String {
  fs::read_to_string(name).unwrap()
}

macro_rules! inc {
  ($e:expr) => {
    Some(read_file(concat!("../deno_typescript/typescript/lib/", $e)))
  };
}

pub fn get_asset(name: &str) -> Option<String> {
  match name {
    "bundle_loader.js" => {
      Some(read_file("../deno_typescript/bundle_loader.js"))
    }
    "lib.deno_core.d.ts" => {
      Some(read_file("../deno_typescript/lib.deno_core.d.ts"))
    }
    "lib.deno_runtime.d.ts" => Some(read_file("js/lib.deno_runtime.d.ts")),
    "bootstrap.ts" => Some("console.log(\"hello deno\");".to_string()),
    "typescript.d.ts" => inc!("typescript.d.ts"),
    "lib.esnext.d.ts" => inc!("lib.esnext.d.ts"),
    "lib.es2020.d.ts" => inc!("lib.es2020.d.ts"),
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
    "lib.es2018.asyncgenerator.d.ts" => inc!("lib.es2018.asyncgenerator.d.ts"),
    "lib.es2018.asynciterable.d.ts" => inc!("lib.es2018.asynciterable.d.ts"),
    "lib.es2018.promise.d.ts" => inc!("lib.es2018.promise.d.ts"),
    "lib.es2018.regexp.d.ts" => inc!("lib.es2018.regexp.d.ts"),
    "lib.es2018.intl.d.ts" => inc!("lib.es2018.intl.d.ts"),
    "lib.es2019.array.d.ts" => inc!("lib.es2019.array.d.ts"),
    "lib.es2019.object.d.ts" => inc!("lib.es2019.object.d.ts"),
    "lib.es2019.string.d.ts" => inc!("lib.es2019.string.d.ts"),
    "lib.es2019.symbol.d.ts" => inc!("lib.es2019.symbol.d.ts"),
    "lib.es2020.string.d.ts" => inc!("lib.es2020.string.d.ts"),
    "lib.es2020.symbol.wellknown.d.ts" => {
      inc!("lib.es2020.symbol.wellknown.d.ts")
    }
    "lib.esnext.array.d.ts" => inc!("lib.esnext.array.d.ts"),
    "lib.esnext.asynciterable.d.ts" => inc!("lib.esnext.asynciterable.d.ts"),
    "lib.esnext.bigint.d.ts" => inc!("lib.esnext.bigint.d.ts"),
    "lib.esnext.intl.d.ts" => inc!("lib.esnext.intl.d.ts"),
    "lib.esnext.symbol.d.ts" => inc!("lib.esnext.symbol.d.ts"),
    _ => None,
  }
}

/// Sets the --trace-serializer V8 flag for debugging snapshots.
pub fn trace_serializer() {
  let dummy = "foo".to_string();
  let r = deno_core::v8_set_flags(vec![
    dummy.clone(),
    "--trace-serializer".to_string(),
  ]);
  assert_eq!(r, vec![dummy]);
}
