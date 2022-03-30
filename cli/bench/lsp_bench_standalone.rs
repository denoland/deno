// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::benchmark_main;
use deno_bench_util::bencher::Bencher;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use test_util::lsp::LspClient;

// Intended to match the benchmark in quick-lint-js
// https://github.com/quick-lint/quick-lint-js/blob/35207e6616267c6c81be63f47ce97ec2452d60df/benchmark/benchmark-lsp/lsp-benchmarks.cpp#L223-L268
fn incremental_change_wait(bench: &mut Bencher) {
  let deno_exe = test_util::deno_exe_path();
  let mut client = LspClient::new(&deno_exe).unwrap();

  static FIXTURE_INIT_JSON: &[u8] =
    include_bytes!("testdata/initialize_params.json");
  let params: Value = serde_json::from_slice(FIXTURE_INIT_JSON).unwrap();
  let (_, maybe_err) = client
    .write_request::<_, _, Value>("initialize", params)
    .unwrap();
  assert!(maybe_err.is_none());
  client.write_notification("initialized", json!({})).unwrap();

  client
    .write_notification(
      "textDocument/didOpen",
      json!({
        "textDocument": {
          "uri": "file:///testdata/express-router.js",
          "languageId": "javascript",
          "version": 0,
          "text": include_str!("testdata/express-router.js")
        }
      }),
    )
    .unwrap();

  let (id, method, _): (u64, String, Option<Value>) =
    client.read_request().unwrap();
  assert_eq!(method, "workspace/configuration");
  client
    .write_response(
      id,
      json!({
        "enable": true
      }),
    )
    .unwrap();

  let (method, _maybe_diag): (String, Option<Value>) =
    client.read_notification().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");

  let mut document_version: u64 = 0;
  bench.iter(|| {
    let text = format!("m{:05}", document_version);
    client
      .write_notification(
        "textDocument/didChange",
        json!({
            "textDocument": {
                "version": document_version,
                "uri":"file:///testdata/express-router.js"
            },
            "contentChanges": [
              {"text": text, "range":{"start":{"line":506,"character":39},"end":{"line":506,"character":45}}},
              {"text": text, "range":{"start":{"line":507,"character":8},"end":{"line":507,"character":14}}},
              {"text": text, "range":{"start":{"line":509,"character":10},"end":{"line":509,"character":16}}}
            ]
        })
    ).unwrap();

     wait_for_deno_lint_diagnostic(document_version, &mut client);

    document_version += 1;
  })
}

fn wait_for_deno_lint_diagnostic(
  document_version: u64,
  client: &mut LspClient,
) {
  loop {
    let (method, maybe_diag): (String, Option<Value>) =
      client.read_notification().unwrap();
    if method == "textDocument/publishDiagnostics" {
      let d = maybe_diag.unwrap();
      let msg = d.as_object().unwrap();
      let version = msg.get("version").unwrap().as_u64().unwrap();
      if document_version == version {
        let diagnostics = msg.get("diagnostics").unwrap().as_array().unwrap();
        for diagnostic in diagnostics {
          let source = diagnostic.get("source").unwrap().as_str().unwrap();
          if source == "deno-lint" {
            return;
          }
        }
      }
    } else {
      todo!() // handle_misc_message
    }
  }
}

benchmark_group!(benches, incremental_change_wait);
benchmark_main!(benches);
