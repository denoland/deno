// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use test_util::env_vars_for_npm_tests;
use test_util::TestContext;
use test_util::TestContextBuilder;

itest!(_036_import_map_fetch {
  args:
    "cache --quiet --reload --import-map=import_maps/import_map.json import_maps/test.ts",
    output: "cache/036_import_map_fetch.out",
  });

itest!(_037_fetch_multiple {
  args: "cache --reload --check=all run/fetch/test.ts run/fetch/other.ts",
  http_server: true,
  output: "cache/037_fetch_multiple.out",
});

itest!(_095_cache_with_bare_import {
  args: "cache cache/095_cache_with_bare_import.ts",
  output: "cache/095_cache_with_bare_import.ts.out",
  exit_code: 1,
});

itest!(cache_extensionless {
  args: "cache --reload --check=all http://localhost:4545/subdir/no_js_ext",
  output: "cache/cache_extensionless.out",
  http_server: true,
});

itest!(cache_random_extension {
  args:
    "cache --reload --check=all http://localhost:4545/subdir/no_js_ext@1.0.0",
  output: "cache/cache_random_extension.out",
  http_server: true,
});

itest!(performance_stats {
  args: "cache --reload --check=all --log-level debug run/002_hello.ts",
  output: "cache/performance_stats.out",
});

itest!(redirect_cache {
  http_server: true,
  args:
    "cache --reload --check=all http://localhost:4548/subdir/redirects/a.ts",
  output: "cache/redirect_cache.out",
});

itest!(ignore_require {
  args: "cache --reload --no-check cache/ignore_require.js",
  output_str: Some(""),
  exit_code: 0,
});

// This test only runs on linux, because it hardcodes the XDG_CACHE_HOME env var
// which is only used on linux.
#[cfg(target_os = "linux")]
#[test]
fn xdg_cache_home_dir() {
  let context = TestContext::with_http_server();
  let deno_dir = context.temp_dir();
  let xdg_cache_home = deno_dir.path().join("cache");
  context
    .new_command()
    .env_remove("HOME")
    .env_remove("DENO_DIR")
    .env_clear()
    .env("XDG_CACHE_HOME", &xdg_cache_home)
    .args(
      "cache --reload --no-check http://localhost:4548/subdir/redirects/a.ts",
    )
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  assert!(xdg_cache_home.read_dir().count() > 0);
}

itest!(check_local_by_default {
  args: "cache --quiet cache/check_local_by_default.ts",
  output: "cache/check_local_by_default.out",
  http_server: true,
});

itest!(check_local_by_default2 {
  args: "cache --quiet cache/check_local_by_default2.ts",
  output: "cache/check_local_by_default2.out",
  http_server: true,
});

itest!(json_import {
  // should not error
  args: "cache --quiet cache/json_import/main.ts",
});

itest!(package_json_basic {
  args: "cache main.ts",
  output: "package_json/basic/main.cache.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  cwd: Some("package_json/basic"),
  copy_temp_dir: Some("package_json/basic"),
  exit_code: 0,
});

#[test]
fn cache_matching_package_json_dep_should_not_install_all() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "package.json",
    r#"{ "dependencies": { "@types/node": "18.8.2", "@denotest/esm-basic": "*" } }"#,
  );
  let output = context
    .new_command()
    .args("cache npm:@types/node@18.8.2")
    .run();
  output.assert_matches_text(concat!(
    "Download http://localhost:4545/npm/registry/@types/node\n",
    "Download http://localhost:4545/npm/registry/@types/node/node-18.8.2.tgz\n",
    "Initialize @types/node@18.8.2\n",
  ));
}

// Regression test for https://github.com/denoland/deno/issues/17299
#[test]
fn cache_put_overwrite() {
  let test_context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();

  let part_one = r#"
  const req = new Request('http://localhost/abc');
  const res1 = new Response('res1');
  const res2 = new Response('res2');

  const cache = await caches.open('test');

  await cache.put(req, res1);
  await cache.put(req, res2);

  const res = await cache.match(req).then((res) => res?.text());
  console.log(res);
    "#;

  let part_two = r#"
  const req = new Request("http://localhost/abc");
  const res1 = new Response("res1");
  const res2 = new Response("res2");

  const cache = await caches.open("test");

  // Swap the order of put() calls.
  await cache.put(req, res2);
  await cache.put(req, res1);

  const res = await cache.match(req).then((res) => res?.text());
  console.log(res);
      "#;

  temp_dir.write("cache_put.js", part_one);

  let run_command =
    test_context.new_command().args_vec(["run", "cache_put.js"]);

  let output = run_command.run();
  output.assert_matches_text("res2\n");
  output.assert_exit_code(0);

  // The wait will surface the bug as we check last written time
  // when we overwrite a response.
  std::thread::sleep(std::time::Duration::from_secs(1));

  temp_dir.write("cache_put.js", part_two);
  let output = run_command.run();
  output.assert_matches_text("res1\n");
  output.assert_exit_code(0);
}

#[test]
fn loads_type_graph() {
  let output = TestContext::default()
    .new_command()
    .args("cache --reload -L debug run/type_directives_js_main.js")
    .run();
  output.assert_matches_text("[WILDCARD] - FileFetcher::fetch() - specifier: file:///[WILDCARD]/subdir/type_reference.d.ts[WILDCARD]");
}
