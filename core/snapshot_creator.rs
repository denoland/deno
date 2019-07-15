// Note: This is a nearly identical rewrite of core/libdeno/snapshot_creator.cc
// but in Rust.
//
// This snapshot program is considered "basic" because the code being
// snapshotted cannot call ops.

extern crate deno;

use deno::js_check;
use deno::Isolate;
use deno::StartupData;
use std::env;
use std::io::Write;

fn main() {
  let args: Vec<String> = env::args().collect();
  // NOTE: `--help` arg will display V8 help and exit
  let args = deno::v8_set_flags(args);

  let (snapshot_out_bin, js_filename) = if args.len() == 3 {
    (args[1].clone(), args[2].clone())
  } else {
    eprintln!("Usage: snapshot_creator <out_filename> <js_filename>");
    std::process::exit(1);
  };

  let js_source =
    std::fs::read(&js_filename).expect("couldn't read js_filename");
  let js_source_str = std::str::from_utf8(&js_source).unwrap();

  let will_snapshot = true;
  let mut isolate = Isolate::new(StartupData::None, will_snapshot);

  js_check(isolate.execute(&js_filename, js_source_str));

  let snapshot = isolate.snapshot().expect("error snapshotting");

  let mut out_file = std::fs::File::create(snapshot_out_bin).unwrap();
  let snapshot_slice =
    unsafe { std::slice::from_raw_parts(snapshot.data_ptr, snapshot.data_len) };
  out_file
    .write_all(snapshot_slice)
    .expect("Failed to write snapshot file");
}
