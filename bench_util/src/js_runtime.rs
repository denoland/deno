use bencher::Bencher;
use deno_core::v8;
use deno_core::JsRuntime;

use crate::profiling::is_profiling;

pub fn create_js_runtime(setup: impl FnOnce(&mut JsRuntime)) -> JsRuntime {
  let mut rt = JsRuntime::new(Default::default());

  // Setup bootstrap namespace
  rt.execute("bootstrap", "globalThis.__bootstrap = {};")
    .unwrap();

  // Caller provided setup
  setup(&mut rt);

  // Init ops
  rt.execute(
    "init",
    r#"
      Deno.core.ops();
      Deno.core.registerErrorClass('Error', Error);
    "#,
  )
  .unwrap();

  rt
}

fn loop_code(iters: u64, src: &str) -> String {
  format!(r#"for(let i=0; i < {}; i++) {{ {} }}"#, iters, src,)
}

pub fn bench_js_sync(
  b: &mut Bencher,
  src: &str,
  setup: impl FnOnce(&mut JsRuntime),
) {
  let mut runtime = create_js_runtime(setup);
  let context = runtime.global_context();
  let scope = &mut v8::HandleScope::with_context(runtime.v8_isolate(), context);

  // Increase JS iterations if profiling for nicer flamegraphs
  let inner_iters = 1000 * if is_profiling() { 10000 } else { 1 };
  // Looped code
  let looped_src = loop_code(inner_iters, src);

  let code = v8::String::new(scope, looped_src.as_ref()).unwrap();
  let script = v8::Script::compile(scope, code, None).unwrap();

  // Run once if profiling, otherwise regular bench loop
  if is_profiling() {
    script.run(scope).unwrap();
  } else {
    b.iter(|| {
      script.run(scope).unwrap();
    });
  }
}

pub fn bench_js_async(
  b: &mut Bencher,
  src: &str,
  setup: impl FnOnce(&mut JsRuntime),
) {
  let mut runtime = create_js_runtime(setup);
  let tokio_runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  // Looped code
  let looped = loop_code(1000, src);
  let src = looped.as_ref();

  if is_profiling() {
    for _ in 0..10000 {
      runtime.execute("inner_loop", src).unwrap();
      let future = runtime.run_event_loop();
      tokio_runtime.block_on(future).unwrap();
    }
  } else {
    b.iter(|| {
      runtime.execute("inner_loop", src).unwrap();
      let future = runtime.run_event_loop();
      tokio_runtime.block_on(future).unwrap();
    });
  }
}
