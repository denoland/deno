// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::Bencher;

use deno_core::Extension;
use deno_core::ExtensionFileSource;
use deno_core::ExtensionFileSourceCode;

fn setup() -> Vec<Extension> {
  vec![
    deno_webidl::deno_webidl::init_ops_and_esm(),
    deno_url::deno_url::init_ops_and_esm(),
    Extension::builder("bench_setup")
      .esm(vec![ExtensionFileSource {
        specifier: "ext:bench_setup/setup",
        code: ExtensionFileSourceCode::IncludedInBinary(
          r#"import { URL } from "ext:deno_url/00_url.js";
        globalThis.URL = URL;
        "#,
        ),
      }])
      .esm_entry_point("ext:bench_setup/setup")
      .build(),
  ]
}

fn bench_url_parse(b: &mut Bencher) {
  bench_js_sync(b, r#"new URL(`http://www.google.com/`);"#, setup);
}

benchmark_group!(benches, bench_url_parse,);
bench_or_profile!(benches);
