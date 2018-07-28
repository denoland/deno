// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use binding::{deno_buf, deno_set_response, DenoC};
use flatbuffers;
use from_c;
use libc::c_char;
use msg_generated::deno as msg;
use std::ffi::CStr;

// Help. Is there a way to do this without macros?
// Want: fn str_from_ptr(*const c_char) -> &str
macro_rules! str_from_ptr {
  ($ptr:expr) => {{
    let cstr = unsafe { CStr::from_ptr($ptr as *const i8) };
    cstr.to_str().unwrap()
  }};
}

/*
// reply_start partially implemented here https://gist.github.com/ry/297c83e0ac8722c045db1b097cdb6afc
pub fn deno_handle_msg_from_js(d: *const DenoC, buf: deno_buf) {
    let s = std::slice::from_raw_parts(buf.data_ptr, buf.data_len);
    buf.data_ptr
    get_root()
}
*/

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
  let module_specifier = str_from_ptr!(module_specifier_);
  let containing_file = str_from_ptr!(containing_file_);

  let deno = from_c(d);

  assert!(deno.dir.root.join("gen") == deno.dir.gen, "Sanity check");

  let result = deno
    .dir
    .code_fetch(module_specifier, containing_file)
    .map_err(|err| {
      let errmsg = format!("{}", err);
      reply_error(d, cmd_id, &errmsg);
    });
  if result.is_err() {
    return;
  }
  let out = result.unwrap();
  // reply_code_fetch
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  let mut msg_args = msg::CodeFetchResArgs {
    module_name: builder.create_string(&out.module_name),
    filename: builder.create_string(&out.filename),
    source_code: builder.create_string(&out.source_code),
    ..Default::default()
  };
  match out.maybe_output_code {
    Some(ref output_code) => {
      msg_args.output_code = builder.create_string(output_code);
    }
    _ => (),
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

// https://github.com/ry/deno/blob/golang/os.go#L156-L169
#[no_mangle]
pub extern "C" fn handle_code_cache(
  d: *const DenoC,
  cmd_id: u32,
  filename_: *const c_char,
  source_code_: *const c_char,
  output_code_: *const c_char,
) {
  let deno = from_c(d);
  let filename = str_from_ptr!(filename_);
  let source_code = str_from_ptr!(source_code_);
  let output_code = str_from_ptr!(output_code_);
  let result = deno.dir.code_cache(filename, source_code, output_code);
  if result.is_err() {
    let err = result.unwrap_err();
    let errmsg = format!("{}", err);
    reply_error(d, cmd_id, &errmsg);
  }
  // null response indicates success.
}
