// Copyright 2018-2025 the Deno authors. MIT license.

use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::benchmark_main;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use test_util::lsp::LspClientBuilder;

// Intended to match the benchmark in quick-lint-js
// https://github.com/quick-lint/quick-lint-js/blob/35207e6616267c6c81be63f47ce97ec2452d60df/benchmark/benchmark-lsp/lsp-benchmarks.cpp#L223-L268
fn incremental_change_wait(bench: &mut Bencher) {
  let mut client = LspClientBuilder::new().build();
  client.initialize_default();
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");
  client.change_configuration(json!({ "deno": { "enable": true } }));
  let (method, _): (String, Option<Value>) = client.read_notification();
  assert_eq!(method, "deno/didRefreshDenoConfigurationTree");

  client.write_notification(
    "textDocument/didOpen",
    json!({
      "textDocument": {
        "uri": "file:///testdata/express-router.js",
        "languageId": "javascript",
        "version": 0,
        "text": include_str!("testdata/express-router.js")
      }
    }),
  );

  client.diagnostic("file:///testdata/express-router.js");

  let mut document_version: u64 = 0;
  bench.iter(|| {
    let text = format!("m{document_version:05}");
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
    );

    client.diagnostic("file:///testdata/express-router.js");

    document_version += 1;
  })
}

benchmark_group!(benches, incremental_change_wait);
benchmark_main!(benches);
