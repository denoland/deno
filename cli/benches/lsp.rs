#[macro_use]
extern crate bencher;
use bencher::Bencher;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;

// Intended to match the benchmark in quick-lint-js
// https://github.com/quick-lint/quick-lint-js/blob/35207e6616267c6c81be63f47ce97ec2452d60df/benchmark/benchmark-lsp/lsp-benchmarks.cpp#L223-L268
fn incremental_change_wait_benchmark(bench: &mut Bencher) {
  let deno_exe = test_util::deno_exe_path();
  let mut client = test_util::lsp::LspClient::new(&deno_exe).unwrap();

  static FIXTURE_INIT_JSON: &[u8] =
    include_bytes!("../bench/testdata/initialize_params.json");
  let params: Value = serde_json::from_slice(FIXTURE_INIT_JSON).unwrap();
  let (_, maybe_err) = client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();
  assert!(maybe_err.is_none());
  client.write_notification("initialized", json!({})).unwrap();

  static FIXTURE_INIT_JSON: &[u8] =
    include_bytes!("../benches/express-router.js");

  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": "file:///testdata/express-router.js",
          "languageId": "javascript",
          "version": 0,
          "text": EXPRESS_JS
        }
      }),
    )
    .unwrap();

  bench.iter(|| {})
}

benchmark_group!(benches, a);
benchmark_main!(benches);
