// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use binding::{deno_buf, deno_set_response, DenoC};
use flatbuffers;
use libc::c_char;
use libc::uint32_t;
use msg_generated::deno as msg;
use std::ffi::CStr;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use url;
use url::Url;

// TODO(ry) SRC_DIR is just a placeholder for future caching functionality.
static SRC_DIR: &str = "/Users/rld/.deno/src/";
const ASSET_PREFIX: &str = "/$asset$/";

#[test]
fn test_url() {
  let issue_list_url = Url::parse("https://github.com/rust-lang").unwrap();
  assert!(issue_list_url.scheme() == "https");
}

fn string_from_ptr(ptr: *const c_char) -> String {
  let cstr = unsafe { CStr::from_ptr(ptr as *const i8) };
  String::from(cstr.to_str().unwrap())
}

/*
// reply_start partially implemented here https://gist.github.com/ry/297c83e0ac8722c045db1b097cdb6afc
pub fn deno_handle_msg_from_js(d: *const DenoC, buf: deno_buf) {
    let s = std::slice::from_raw_parts(buf.data_ptr, buf.data_len);
    buf.data_ptr
    get_root()
}
*/

// Prototype: https://github.com/ry/deno/blob/golang/os.go#L56-L68
#[allow(dead_code)]
fn src_file_to_url<P: AsRef<Path>>(filename: P) -> String {
  assert!(SRC_DIR.len() > 0, "SRC_DIR shouldn't be empty");

  let filename = filename.as_ref().to_path_buf();
  let src = (SRC_DIR.as_ref() as &Path).to_path_buf();

  if filename.starts_with(&src) {
    let rest = filename.strip_prefix(&src).unwrap();
    "http://".to_string() + rest.to_str().unwrap()
  } else {
    String::from(filename.to_str().unwrap())
  }
}

#[test]
fn test_src_file_to_url() {
  assert_eq!("hello", src_file_to_url("hello"));
  assert_eq!("/hello", src_file_to_url("/hello"));
  let x = SRC_DIR.to_string() + "hello";
  assert_eq!("http://hello", src_file_to_url(&x));
  let x = SRC_DIR.to_string() + "/hello";
  assert_eq!("http://hello", src_file_to_url(&x));
}

// Prototype: https://github.com/ry/deno/blob/golang/os.go#L70-L98
// Returns (module name, local filename)
fn resolve_module(
  module_specifier: &String,
  containing_file: &String,
) -> Result<(String, String), url::ParseError> {
  info!(
    "resolve_module before module_specifier {} containing_file {}",
    module_specifier, containing_file
  );

  //let module_specifier = src_file_to_url(module_specifier);
  //let containing_file = src_file_to_url(containing_file);
  //let base_url = Url::parse(&containing_file)?;

  let j: Url =
    if containing_file == "." || Path::new(module_specifier).is_absolute() {
      Url::from_file_path(module_specifier).unwrap()
    } else if containing_file.as_str().ends_with("/") {
      let base = Url::from_directory_path(&containing_file).unwrap();
      base.join(module_specifier)?
    } else {
      let base = Url::from_file_path(&containing_file).unwrap();
      base.join(module_specifier)?
    };

  let mut p = j.to_file_path()
    .unwrap()
    .into_os_string()
    .into_string()
    .unwrap();

  if cfg!(target_os = "windows") {
    // On windows, replace backward slashes to forward slashes.
    // TODO(piscisaureus): This may not me be right, I just did it to make
    // the tests pass.
    p = p.replace("\\", "/");
  }

  let module_name = p.to_string();
  let filename = p.to_string();

  Ok((module_name, filename))
}

// https://github.com/ry/deno/blob/golang/os_test.go#L16-L87
#[test]
fn test_resolve_module() {
  // The `add_root` macro prepends "C:" to a string if on windows; on posix
  // systems it returns the input string untouched. This is necessary because
  // `Url::from_file_path()` fails if the input path isn't an absolute path.
  macro_rules! add_root {
    ($path:expr) => {
      if cfg!(target_os = "windows") {
        concat!("C:", $path)
      } else {
        $path
      }
    };
  }

  let test_cases = [
    (
      "./subdir/print_hello.ts",
      add_root!(
        "/Users/rld/go/src/github.com/ry/deno/testdata/006_url_imports.ts"
      ),
      add_root!(
        "/Users/rld/go/src/github.com/ry/deno/testdata/subdir/print_hello.ts"
      ),
      add_root!(
        "/Users/rld/go/src/github.com/ry/deno/testdata/subdir/print_hello.ts"
      ),
    ),
    (
      "testdata/001_hello.js",
      add_root!("/Users/rld/go/src/github.com/ry/deno/"),
      add_root!("/Users/rld/go/src/github.com/ry/deno/testdata/001_hello.js"),
      add_root!("/Users/rld/go/src/github.com/ry/deno/testdata/001_hello.js"),
    ),
    (
      add_root!("/Users/rld/src/deno/hello.js"),
      ".",
      add_root!("/Users/rld/src/deno/hello.js"),
      add_root!("/Users/rld/src/deno/hello.js"),
    ),
    (
      add_root!("/this/module/got/imported.js"),
      add_root!("/that/module/did/it.js"),
      add_root!("/this/module/got/imported.js"),
      add_root!("/this/module/got/imported.js"),
    ),
    /*
        (
            "http://localhost:4545/testdata/subdir/print_hello.ts",
            add_root!("/Users/rld/go/src/github.com/ry/deno/testdata/006_url_imports.ts"),
            "http://localhost:4545/testdata/subdir/print_hello.ts",
            path.Join(SrcDir, "localhost:4545/testdata/subdir/print_hello.ts"),
        ),
        (
            path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"),
            ".",
            "http://unpkg.com/liltest@0.0.5/index.ts",
            path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"),
        ),
        (
            "./util",
            path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"),
            "http://unpkg.com/liltest@0.0.5/util",
            path.Join(SrcDir, "unpkg.com/liltest@0.0.5/util"),
        ),
        */
  ];
  for &test in test_cases.iter() {
    let module_specifier = String::from(test.0);
    let containing_file = String::from(test.1);
    let (module_name, filename) =
      resolve_module(&module_specifier, &containing_file).unwrap();
    assert_eq!(module_name, test.2);
    assert_eq!(filename, test.3);
  }
}

pub fn reply_code_fetch(
  d: *const DenoC,
  cmd_id: uint32_t,
  module_name: &String,
  filename: &String,
  source_code: &String,
  output_code: &String,
) {
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  let msg_args = msg::CodeFetchResArgs {
    module_name: builder.create_string(module_name),
    filename: builder.create_string(filename),
    source_code: builder.create_string(source_code),
    output_code: builder.create_string(output_code),
    ..Default::default()
  };
  let msg = msg::CreateCodeFetchRes(&mut builder, &msg_args);
  builder.finish(msg);
  let args = msg::BaseArgs {
    cmdId: cmd_id,
    msg: Some(msg.union()),
    msg_type: msg::Any::CodeFetchRes,
    ..Default::default()
  };
  set_response_base(d, &mut builder, &args)
}

fn reply_error(d: *const DenoC, cmd_id: u32, msg: &String) {
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  // println!("reply_error{}", msg);
  let args = msg::BaseArgs {
    cmdId: cmd_id,
    error: builder.create_string(msg),
    ..Default::default()
  };
  set_response_base(d, &mut builder, &args)
}

fn set_response_base(
  d: *const DenoC,
  builder: &mut flatbuffers::FlatBufferBuilder,
  args: &msg::BaseArgs,
) {
  let base = msg::CreateBase(builder, &args);
  builder.finish(base);
  let data = builder.get_active_buf_slice();
  // println!("buf slice {} {} {} {} {}", data[0], data[1], data[2], data[3], data[4]);
  let buf = deno_buf {
    // TODO(ry)
    // The deno_buf / ImportBuf / ExportBuf semantics should be such that we do not need to yield
    // ownership. Temporarally there is a hack in ImportBuf that when alloc_ptr is null, it will
    // memcpy the deno_buf into V8 instead of doing zero copy.
    alloc_ptr: 0 as *mut u8,
    alloc_len: 0,
    data_ptr: data.as_ptr() as *mut u8,
    data_len: data.len(),
  };
  // println!("data_ptr {:p}", data_ptr);
  // println!("data_len {}", data.len());
  unsafe { deno_set_response(d, buf) }
}

// https://github.com/ry/deno/blob/golang/os.go#L100-L154
#[no_mangle]
pub extern "C" fn handle_code_fetch(
  d: *const DenoC,
  cmd_id: u32,
  module_specifier_: *const c_char,
  containing_file_: *const c_char,
) {
  let module_specifier = string_from_ptr(module_specifier_);
  let containing_file = string_from_ptr(containing_file_);

  let result = resolve_module(&module_specifier, &containing_file);
  if result.is_err() {
    let err = result.unwrap_err();
    let errmsg = format!("{} {} {}", err, module_specifier, containing_file);
    reply_error(d, cmd_id, &errmsg);
    return;
  }
  let (module_name, filename) = result.unwrap();

  let mut source_code = String::new();

  debug!(
        "code_fetch. module_name = {} module_specifier = {} containing_file = {} filename = {}",
        module_name, module_specifier, containing_file, filename
    );

  if is_remote(&module_name) {
    unimplemented!();
  } else if module_name.starts_with(ASSET_PREFIX) {
    assert!(false, "Asset resolution should be done in JS, not Rust.");
  } else {
    assert!(
      module_name == filename,
      "if a module isn't remote, it should have the same filename"
    );
    let result = File::open(&filename);
    if result.is_err() {
      let err = result.unwrap_err();
      let errmsg = format!("{} {}", err, filename);
      reply_error(d, cmd_id, &errmsg);
      return;
    }
    let mut f = result.unwrap();
    let result = f.read_to_string(&mut source_code);
    if result.is_err() {
      let err = result.unwrap_err();
      let errmsg = format!("{} {}", err, filename);
      reply_error(d, cmd_id, &errmsg);
      return;
    }
  }

  let output_code = String::new(); //load_output_code_cache(filename, source_code);

  reply_code_fetch(
    d,
    cmd_id,
    &module_name,
    &filename,
    &source_code,
    &output_code,
  )
}

fn is_remote(_module_name: &String) -> bool {
  false
}
