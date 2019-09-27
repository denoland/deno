/// To run this benchmark:
///
/// > DENO_BUILD_MODE=release ./tools/build.py && \
///   ./target/release/deno_core_http_bench --multi-thread
extern crate deno;
extern crate futures;
extern crate tokio;

use deno::*;
use futures::future::lazy;
use tokio::prelude::*;


fn main() {
  let main_future = lazy(move || {
    // TODO currently isolate.execute() must be run inside tokio, hence the
    // lazy(). It would be nice to not have that contraint. Probably requires
    // using v8::MicrotasksPolicy::kExplicit

    let js_source = include_str!("hello.js");

    let mut isolate = deno::Isolate::new(StartupData::None, false);
    let r = op_hello_js::init(&mut isolate);
    eprintln!("result r {:?}", r);
    let r = isolate.execute("hello.js", js_source);
    eprintln!("result r {:?}", r);

    isolate.then(|r| {
      js_check(r);
      Ok(())
    })
  });

  tokio::runtime::current_thread::run(main_future);
}

fn js_check(r: Result<(), ErrBox>) {
  if let Err(e) = r {
    panic!(e.to_string());
  }
}
