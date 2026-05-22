// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  // 22_body.js loads 21_formdata.js via `core.loadExtScript`, so the
  // formdata source has to be registered as lazy-loaded.
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = [
      "ext:deno_fetch/22_body.js" = "22_body.js",
      "ext:bench_setup/setup" = {
        source = r#"
          import "ext:deno_fetch/22_body.js";
          const { Blob } =
            __bootstrap.core.loadExtScript("ext:deno_web/09_file.js");
          const { FormData, formDataToBlob } =
            __bootstrap.core.loadExtScript("ext:deno_fetch/21_formdata.js");

          // 4 small text fields (login form / small API call shape).
          const fdSmall = new FormData();
          fdSmall.append("user", "alice");
          fdSmall.append("op", "create");
          fdSmall.append("amount", "42");
          fdSmall.append("currency", "USD");

          // 10 small text fields.
          const fdManyText = new FormData();
          for (let i = 0; i < 10; i++) {
            fdManyText.append("field_" + i, "value_with_some_content_" + i);
          }

          // 3 text + 1 small Blob (typical multipart upload).
          const fdWithFile = new FormData();
          fdWithFile.append("name", "alice");
          fdWithFile.append("op", "upload");
          fdWithFile.append(
            "file",
            new Blob(["small file content"], { type: "text/plain" }),
            "test.txt",
          );
          fdWithFile.append("notes", "uploaded by alice");

          globalThis.benchSmall = () => formDataToBlob(fdSmall);
          globalThis.benchManyText = () => formDataToBlob(fdManyText);
          globalThis.benchWithFile = () => formDataToBlob(fdWithFile);
        "#
      },
    ],
    lazy_loaded_js = ["ext:deno_fetch/21_formdata.js" = "21_formdata.js",],
  );

  vec![
    deno_webidl::deno_webidl::init(),
    deno_web::deno_web::init(
      Default::default(),
      None,
      Default::default(),
      Default::default(),
    ),
    bench_setup::init(),
  ]
}

fn bench_form_data_to_blob_4_text(b: &mut Bencher) {
  bench_js_sync(b, r#"benchSmall();"#, setup);
}

fn bench_form_data_to_blob_10_text(b: &mut Bencher) {
  bench_js_sync(b, r#"benchManyText();"#, setup);
}

fn bench_form_data_to_blob_text_and_file(b: &mut Bencher) {
  bench_js_sync(b, r#"benchWithFile();"#, setup);
}

benchmark_group!(
  benches,
  bench_form_data_to_blob_4_text,
  bench_form_data_to_blob_10_text,
  bench_form_data_to_blob_text_and_file,
);
bench_or_profile!(benches);
