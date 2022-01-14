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
  let (method, _maybe_diag): (String, Option<Value>) =
    client.read_notification().unwrap();
  assert_eq!(method, "textDocument/publishDiagnostics");
  //let _expected_num_diagnostics = get_num_diagnostics(maybe_diag);

  let mut document_version: u64 = 0;
  bench.iter(|| {
      println!("document_version {}", document_version);
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

      const DIAGNOSTICS_MESSAGES_TO_IGNORE: usize = 1;
      for _ in 0..DIAGNOSTICS_MESSAGES_TO_IGNORE {
          wait_for_first_diagnostics_notification(document_version, &mut client);
      }
       wait_for_first_diagnostics_notification(document_version, &mut client);

      document_version += 1;
    })
}

fn wait_for_first_diagnostics_notification(
  document_version: u64,
  client: &mut LspClient,
) -> Value {
  loop {
    let (method, maybe_diag): (String, Option<Value>) =
      client.read_notification().unwrap();
    if method == "textDocument/publishDiagnostics" {
      let d = maybe_diag.unwrap();
      if document_version == get_diagnostic_version(&d) {
        return d;
      }
    } else {
      // handle_misc_message
      todo!()
    }
  }
}

fn get_diagnostic_version(diag: &Value) -> u64 {
  let msg = diag.as_object().unwrap();
  msg.get("version").unwrap().as_u64().unwrap()
}

/*
fn get_num_diagnostics(diag: &Value) -> usize {
  let msg = diag.as_object().unwrap();
  msg.get("diagnostics").unwrap().as_array().unwrap().len()
}
*/

benchmark_group!(benches, incremental_change_wait);
benchmark_main!(benches);
