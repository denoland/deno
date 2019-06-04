mod libdeno;

use libc::c_void;
use std;
use std::env;
use std::ffi::CStr;
use std::ffi::CString;
use std::io::Write;
use std::ptr;

extern "C" fn config_cb(
  _user_data: *mut c_void,
  _control_argv0: libdeno::deno_buf,
  _zero_copy_buf: libdeno::deno_pinned_buf,
) {
}

fn main() {
  println!("hello world from snapshot creator in rust");

  // TODO: set v8 flags
  let args: Vec<String> = env::args().collect();

  let (snapshot_out_bin, js_filename) = if args.len() == 2 {
    (args[0].clone(), args[1].clone())
  } else {
    eprintln!("Usage: snapshopt_creator <out_dir> <js_filename>");
    std::process::exit(1);
  };

  let js_source = match std::fs::read(&js_filename) {
    Ok(source) => source.to_owned(),
    _ => panic!(
      "Error retrieving source file at \"{}\"",
      js_filename.as_str()
    ),
  };

  unsafe { libdeno::deno_init() };

  let libdeno_config = libdeno::deno_config {
    will_snapshot: 1,
    load_snapshot: libdeno::Snapshot1::empty(),
    shared: libdeno::deno_buf::empty(),
    recv_cb: config_cb,
  };

  let isolate = unsafe { libdeno::deno_new(libdeno_config) };

  let filename = CString::new(js_filename).unwrap();
  let source = CString::new(js_source).unwrap();

  unsafe {
    libdeno::deno_execute(
      isolate,
      ptr::null(),
      filename.as_ptr(),
      source.as_ptr(),
    )
  };

  let ptr = unsafe { libdeno::deno_last_exception(isolate) };
  if !ptr.is_null() {
    let cstr = unsafe { CStr::from_ptr(ptr) };
    let v8_exception = cstr.to_str().unwrap();
    eprintln!("Snapshot exception\n{}\n", v8_exception);
    unsafe {
      libdeno::deno_delete(isolate);
    }
    std::process::exit(1);
  }

  let mut snapshot = unsafe { libdeno::deno_snapshot_new(isolate) };

  let mut out_file = std::fs::File::create(snapshot_out_bin).unwrap();
  let snapshot_slice =
    unsafe { std::slice::from_raw_parts(snapshot.data_ptr, snapshot.data_len) };
  out_file.write_all(snapshot_slice);

  unsafe {
    libdeno::deno_snapshot_delete(&mut snapshot);
    libdeno::deno_delete(isolate);
  }
}
