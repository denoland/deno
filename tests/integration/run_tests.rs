// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;

use bytes::Bytes;
use hickory_proto::serialize::txt::Parser;
use hickory_server::authority::AuthorityObject;
use pretty_assertions::assert_eq;
use rustls::ClientConnection;
use rustls_tokio_stream::TlsStream;
use serde_json::json;
use test_util as util;
use test_util::itest;
use test_util::TempDir;
use util::assert_contains;
use util::assert_not_contains;
use util::PathRef;
use util::TestContext;
use util::TestContextBuilder;

const CODE_CACHE_DB_FILE_NAME: &str = "v8_code_cache_v2";

// tests to ensure that when `--location` is set, all code shares the same
// localStorage cache based on the origin of the location URL.
#[test]
fn webstorage_location_shares_origin() {
  let deno_dir = util::new_deno_dir();

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--location")
    .arg("https://example.com/a.ts")
    .arg("run/webstorage/fixture.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 0 }\n");

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--location")
    .arg("https://example.com/b.ts")
    .arg("run/webstorage/logger.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { hello: \"deno\", length: 1 }\n");
}

// test to ensure that when a --config file is set, but no --location, that
// storage persists against unique configuration files.
#[test]
fn webstorage_config_file() {
  let context = TestContext::default();

  context
    .new_command()
    .args(
      "run --config run/webstorage/config_a.jsonc run/webstorage/fixture.ts",
    )
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context
    .new_command()
    .args("run --config run/webstorage/config_b.jsonc run/webstorage/logger.ts")
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context
    .new_command()
    .args("run --config run/webstorage/config_a.jsonc run/webstorage/logger.ts")
    .run()
    .assert_matches_text("Storage { hello: \"deno\", length: 1 }\n");
}

// tests to ensure `--config` does not effect persisted storage when a
// `--location` is provided.
#[test]
fn webstorage_location_precedes_config() {
  let context = TestContext::default();

  context.new_command()
    .args("run --location https://example.com/a.ts --config run/webstorage/config_a.jsonc run/webstorage/fixture.ts")
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context.new_command()
    .args("run --location https://example.com/b.ts --config run/webstorage/config_b.jsonc run/webstorage/logger.ts")
    .run()
    .assert_matches_text("Storage { hello: \"deno\", length: 1 }\n");
}

// test to ensure that when there isn't a configuration or location, that the
// main module is used to determine how to persist storage data.
#[test]
fn webstorage_main_module() {
  let context = TestContext::default();

  context
    .new_command()
    .args("run run/webstorage/fixture.ts")
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context
    .new_command()
    .args("run run/webstorage/logger.ts")
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context
    .new_command()
    .args("run run/webstorage/fixture.ts")
    .run()
    .assert_matches_text("Storage { hello: \"deno\", length: 1 }\n");
}

#[test]
fn _083_legacy_external_source_map() {
  let _g = util::http_server();
  let deno_dir = TempDir::new();
  let module_url = url::Url::parse(
    "http://localhost:4545/run/083_legacy_external_source_map.ts",
  )
  .unwrap();
  // Write a faulty old external source map.
  let faulty_map_path = deno_dir.path().join("gen/http/localhost_PORT4545/9576bd5febd0587c5c4d88d57cb3ac8ebf2600c529142abe3baa9a751d20c334.js.map");
  faulty_map_path.parent().create_dir_all();
  faulty_map_path.write(r#"{\"version\":3,\"file\":\"\",\"sourceRoot\":\"\",\"sources\":[\"http://localhost:4545/083_legacy_external_source_map.ts\"],\"names\":[],\"mappings\":\";AAAA,MAAM,IAAI,KAAK,CAAC,KAAK,CAAC,CAAC\"}"#);
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(module_url.to_string())
    .output()
    .unwrap();
  // Before https://github.com/denoland/deno/issues/6965 was fixed, the faulty
  // old external source map would cause a panic while formatting the error
  // and the exit code would be 101. The external source map should be ignored
  // in favor of the inline one.
  assert_eq!(output.status.code(), Some(1));
  let out = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(out, "");
}

itest!(_089_run_allow_list {
  args: "run --allow-run=curl run/089_run_allow_list.ts",
  envs: vec![
    ("LD_LIBRARY_PATH".to_string(), "".to_string()),
    ("DYLD_FALLBACK_LIBRARY_PATH".to_string(), "".to_string())
  ],
  output: "run/089_run_allow_list.ts.out",
});

#[test]
fn _090_run_permissions_request() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/090_run_permissions_request.ts"])
    .with_pty(|mut console| {
      console.expect(concat!(
        "┏ ⚠️  Deno requests run access to \"ls\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-run\r\n",
        "┠─ Run again with --allow-run to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.human_delay();
      console.write_line_raw("y");
      console.expect("Granted run access to \"ls\".");
      console.expect(concat!(
        "┏ ⚠️  Deno requests run access to \"cat\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-run\r\n",
        "┠─ Run again with --allow-run to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.human_delay();
      console.write_line_raw("n");
      console.expect("Denied run access to \"cat\".");
      console.expect("granted");
      console.expect("denied");
    });
}

#[test]
fn _090_run_permissions_request_sync() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/090_run_permissions_request_sync.ts"])
    .with_pty(|mut console| {
      console.expect(concat!(
        "┏ ⚠️  Deno requests run access to \"ls\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-run\r\n",
        "┠─ Run again with --allow-run to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.human_delay();
      console.write_line_raw("y");
      console.expect("Granted run access to \"ls\".");
      console.expect(concat!(
        "┏ ⚠️  Deno requests run access to \"cat\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-run\r\n",
        "┠─ Run again with --allow-run to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.human_delay();
      console.write_line_raw("n");
      console.expect("Denied run access to \"cat\".");
      console.expect("granted");
      console.expect("denied");
    });
}

#[test]
fn permissions_prompt_allow_all() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permissions_prompt_allow_all.ts"])
    .with_pty(|mut console| {
      // "run" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests run access to \"FOO\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-run\r\n",
        "┠─ Run again with --allow-run to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all run access.");
      // "read" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests read access to \"FOO\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
        "┠─ Run again with --allow-read to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all read access.");
      // "write" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests write access to \"FOO\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-write\r\n",
        "┠─ Run again with --allow-write to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all write permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all write access.");
      // "net" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests net access to \"foo\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-net\r\n",
        "┠─ Run again with --allow-net to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all net permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all net access.");
      // "env" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests env access to \"FOO\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-env\r\n",
        "┠─ Run again with --allow-env to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all env permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all env access.");
      // "sys" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests sys access to \"loadavg\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-sys\r\n",
        "┠─ Run again with --allow-sys to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all sys permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all sys access.");
      // "ffi" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests ffi access to \"FOO\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-ffi\r\n",
        "┠─ Run again with --allow-ffi to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all ffi permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all ffi access.")
    },
  );
}

#[flaky_test::flaky_test]
fn permissions_prompt_allow_all_2() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permissions_prompt_allow_all_2.ts"])
    .with_pty(|mut console| {
      // "env" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests env access to \"FOO\".\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-env\r\n",
        "┠─ Run again with --allow-env to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all env permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all env access.");

      // "sys" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests sys access to \"loadavg\".\r\n",
        "┠─ Requested by `Deno.loadavg()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-sys\r\n",
        "┠─ Run again with --allow-sys to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all sys permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all sys access.");

      let text = console.read_until("Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)");
      // "read" permissions
      test_util::assertions::assert_wildcard_match(&text, concat!(
        "\r\n",
        "┏ ⚠️  Deno requests read access to \"[WILDCARD]tests[WILDCHAR]testdata[WILDCHAR]\".\r\n",
        "┠─ Requested by `Deno.lstatSync()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
        "┠─ Run again with --allow-read to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
      ));
      console.human_delay();
      console.write_line_raw("A");
      console.expect("Granted all read access.");
    });
}

#[test]
fn permissions_prompt_allow_all_lowercase_a() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permissions_prompt_allow_all.ts"])
    .with_pty(|mut console| {
      // "run" permissions
      console.expect(concat!(
        "┏ ⚠️  Deno requests run access to \"FOO\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-run\r\n",
        "┠─ Run again with --allow-run to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.human_delay();
      console.write_line_raw("a");
      console.expect("Unrecognized option.");
    });
}

#[test]
fn permission_request_long() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permission_request_long.ts"])
    .with_pty(|mut console| {
      console.expect(concat!(
        "was larger than the configured maximum length (10240 bytes): denying request.\r\n",
        "❌ WARNING: This may indicate that code is trying to bypass or hide permission check requests.\r\n",
        "❌ Run again with --allow-read to bypass this check if this is really what you want to do.\r\n",
      ));
    });
}

#[test]
fn permissions_cache() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permissions_cache.ts"])
    .with_pty(|mut console| {
      console.expect(concat!(
        "prompt\r\n",
        "┏ ⚠️  Deno requests read access to \"foo\".\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
        "┠─ Run again with --allow-read to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
      ));
      console.human_delay();
      console.write_line_raw("y");
      console.expect("Granted read access to \"foo\".");
      console.expect("granted");
      console.expect("prompt");
    });
}

#[test]
fn permissions_trace() {
  TestContext::default()
    .new_command()
    .env("DENO_TRACE_PERMISSIONS", "1")
    .args_vec(["run", "--quiet", "run/permissions_trace.ts"])
    .with_pty(|mut console| {
      let text = console.read_until("Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all sys permissions)");
      test_util::assertions::assert_wildcard_match(&text, concat!(
      "┏ ⚠️  Deno requests sys access to \"hostname\".\r\n",
      "┠─ Requested by `Deno.hostname()` API.\r\n",
      "┃  ├─ Object.hostname (ext:deno_os/30_os.js:43:10)\r\n",
      "┃  ├─ foo (file://[WILDCARD]/run/permissions_trace.ts:2:8)\r\n",
      "┃  ├─ bar (file://[WILDCARD]/run/permissions_trace.ts:6:3)\r\n",
      "┃  └─ file://[WILDCARD]/run/permissions_trace.ts:9:1\r\n",
      "┠─ Learn more at: https://docs.deno.com/go/--allow-sys\r\n",
      "┠─ Run again with --allow-sys to bypass this prompt.\r\n",
      "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all sys permissions)",
      ));

      console.human_delay();
      console.write_line_raw("y");
      console.expect("Granted sys access to \"hostname\".");
    });
}

itest!(lock_write_fetch {
  args:
    "run --quiet --allow-import --allow-read --allow-write --allow-env --allow-run run/lock_write_fetch/main.ts",
  output: "run/lock_write_fetch/main.out",
  http_server: true,
  exit_code: 0,
});

#[test]
fn lock_redirects() {
  let context = TestContextBuilder::new()
    .use_temp_cwd()
    .use_http_server()
    .add_npm_env_vars()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", "{}"); // cause a lockfile to be created
  temp_dir.write(
    "main.ts",
    "import 'http://localhost:4546/run/001_hello.js';",
  );
  context
    .new_command()
    .args("run --allow-import main.ts")
    .run()
    .skip_output_check();
  let initial_lockfile_text = r#"{
  "version": "5",
  "redirects": {
    "http://localhost:4546/run/001_hello.js": "http://localhost:4545/run/001_hello.js"
  },
  "remote": {
    "http://localhost:4545/run/001_hello.js": "c479db5ea26965387423ca438bb977d0b4788d5901efcef52f69871e4c1048c5"
  }
}
"#;
  assert_eq!(temp_dir.read_to_string("deno.lock"), initial_lockfile_text);
  context
    .new_command()
    .args("run --allow-import main.ts")
    .run()
    .assert_matches_text("Hello World\n");
  assert_eq!(temp_dir.read_to_string("deno.lock"), initial_lockfile_text);

  // now try changing where the redirect occurs in the lockfile
  temp_dir.write("deno.lock", r#"{
  "version": "5",
  "redirects": {
    "http://localhost:4546/run/001_hello.js": "http://localhost:4545/echo.ts"
  },
  "remote": {
    "http://localhost:4545/run/001_hello.js": "c479db5ea26965387423ca438bb977d0b4788d5901efcef52f69871e4c1048c5"
  }
}
"#);

  // also, add some npm dependency to ensure it doesn't end up in
  // the redirects as they're currently stored separately
  temp_dir.write(
    "main.ts",
    "import 'http://localhost:4546/run/001_hello.js';\n import 'npm:@denotest/esm-basic';\n",
  );

  // it should use the echo script instead
  context
    .new_command()
    .args("run --allow-import main.ts Hi there")
    .run()
    .assert_matches_text(concat!(
      "Download http://localhost:4545/echo.ts\n",
      "Download http://localhost:4260/@denotest%2fesm-basic\n",
      "Download http://localhost:4260/@denotest/esm-basic/1.0.0.tgz\n",
      "Hi, there",
    ));
  util::assertions::assert_wildcard_match(
    &temp_dir.read_to_string("deno.lock"),
    r#"{
  "version": "5",
  "specifiers": {
    "npm:@denotest/esm-basic@*": "1.0.0"
  },
  "npm": {
    "@denotest/esm-basic@1.0.0": {
      "integrity": "sha512-[WILDCARD]"
    }
  },
  "redirects": {
    "http://localhost:4546/run/001_hello.js": "http://localhost:4545/echo.ts"
  },
  "remote": {
    "http://localhost:4545/echo.ts": "829eb4d67015a695d70b2a33c78b631b29eea1dbac491a6bfcf394af2a2671c2",
    "http://localhost:4545/run/001_hello.js": "c479db5ea26965387423ca438bb977d0b4788d5901efcef52f69871e4c1048c5"
  }
}
"#,
  );
}

#[test]
fn lock_deno_json_package_json_deps() {
  let context = TestContextBuilder::new()
    .use_temp_cwd()
    .use_http_server()
    .add_npm_env_vars()
    .add_jsr_env_vars()
    .build();
  let temp_dir = context.temp_dir().path();
  let deno_json = temp_dir.join("deno.json");
  let package_json = temp_dir.join("package.json");

  // add a jsr and npm dependency
  deno_json.write_json(&json!({
    "nodeModulesDir": "auto",
    "imports": {
      "esm-basic": "npm:@denotest/esm-basic",
      "module_graph": "jsr:@denotest/module-graph@1.4",
    }
  }));
  let main_ts = temp_dir.join("main.ts");
  main_ts.write("import 'esm-basic'; import 'module_graph';");
  context
    .new_command()
    .args("cache main.ts")
    .run()
    .skip_output_check();
  let lockfile = temp_dir.join("deno.lock");
  let esm_basic_integrity =
    get_lockfile_npm_package_integrity(&lockfile, "@denotest/esm-basic@1.0.0");
  lockfile.assert_matches_json(json!({
    "version": "5",
    "specifiers": {
      "jsr:@denotest/module-graph@1.4": "1.4.0",
      "npm:@denotest/esm-basic@*": "1.0.0"
    },
    "jsr": {
      "@denotest/module-graph@1.4.0": {
        "integrity": "32de0973c5fa55772326fcd504a757f386d2b010db3e13e78f3bcf851e69473d"
      }
    },
    "npm": {
      "@denotest/esm-basic@1.0.0": {
        "integrity": esm_basic_integrity,
        "tarball": "http://localhost:4260/@denotest/esm-basic/1.0.0.tgz"
      }
    },
    "workspace": {
      "dependencies": [
        "jsr:@denotest/module-graph@1.4",
        "npm:@denotest/esm-basic@*"
      ]
    }
  }));

  // now remove the npm dependency from the deno.json and move
  // it to a package.json that uses an alias
  deno_json.write_json(&json!({
    "nodeModulesDir": "auto",
    "imports": {
      "module_graph": "jsr:@denotest/module-graph@1.4",
    }
  }));
  package_json.write_json(&json!({
    "dependencies": {
      "esm-basic": "npm:@denotest/esm-basic"
    }
  }));
  context
    .new_command()
    .args("cache main.ts")
    .run()
    .skip_output_check();
  main_ts.write("import 'module_graph';");
  context
    .new_command()
    // ensure this doesn't clear out packageJson below
    .args("cache --no-npm main.ts")
    .run()
    .skip_output_check();
  lockfile.assert_matches_json(json!({
    "version": "5",
    "specifiers": {
      "jsr:@denotest/module-graph@1.4": "1.4.0",
      "npm:@denotest/esm-basic@*": "1.0.0"
    },
    "jsr": {
      "@denotest/module-graph@1.4.0": {
        "integrity": "32de0973c5fa55772326fcd504a757f386d2b010db3e13e78f3bcf851e69473d"
      }
    },
    "npm": {
      "@denotest/esm-basic@1.0.0": {
        "integrity": esm_basic_integrity,
        "tarball": "http://localhost:4260/@denotest/esm-basic/1.0.0.tgz"
      }
    },
    "workspace": {
      "dependencies": [
        "jsr:@denotest/module-graph@1.4"
      ],
      "packageJson": {
        "dependencies": [
          "npm:@denotest/esm-basic@*"
        ]
      }
    }
  }));

  // now remove the package.json
  package_json.remove_file();

  // cache and it will remove the package.json
  context
    .new_command()
    .args("cache main.ts")
    .run()
    .skip_output_check();
  lockfile.assert_matches_json(json!({
    "version": "5",
    "specifiers": {
      "jsr:@denotest/module-graph@1.4": "1.4.0",
    },
    "jsr": {
      "@denotest/module-graph@1.4.0": {
        "integrity": "32de0973c5fa55772326fcd504a757f386d2b010db3e13e78f3bcf851e69473d"
      }
    },
    "workspace": {
      "dependencies": [
        "jsr:@denotest/module-graph@1.4"
      ]
    }
  }));

  // now remove the deps from the deno.json
  deno_json.write_json(&json!({
    "nodeModulesDir": "auto"
  }));
  main_ts.write("");
  context
    .new_command()
    .args("cache main.ts")
    .run()
    .skip_output_check();

  lockfile.assert_matches_json(json!({
    "version": "5"
  }));
}

#[test]
fn lock_deno_json_package_json_deps_workspace() {
  let context = TestContextBuilder::new()
    .use_temp_cwd()
    .use_http_server()
    .add_npm_env_vars()
    .add_jsr_env_vars()
    .build();
  let temp_dir = context.temp_dir().path();

  // deno.json
  let deno_json = temp_dir.join("deno.json");
  deno_json.write_json(&json!({
    "nodeModulesDir": "auto"
  }));

  // package.json
  let package_json = temp_dir.join("package.json");
  package_json.write_json(&json!({
    "workspaces": ["package-a"],
    "dependencies": {
      "@denotest/cjs-default-export": "1"
    }
  }));
  // main.ts
  let main_ts = temp_dir.join("main.ts");
  main_ts.write("import '@denotest/cjs-default-export';");

  // package-a/package.json
  let a_package = temp_dir.join("package-a");
  a_package.create_dir_all();
  let a_package_json = a_package.join("package.json");
  a_package_json.write_json(&json!({
    "dependencies": {
      "@denotest/esm-basic": "1"
    }
  }));
  // package-a/main.ts
  let main_ts = a_package.join("main.ts");
  main_ts.write("import '@denotest/esm-basic';");
  context
    .new_command()
    .args("run package-a/main.ts")
    .run()
    .skip_output_check();
  let lockfile = temp_dir.join("deno.lock");
  let esm_basic_integrity =
    get_lockfile_npm_package_integrity(&lockfile, "@denotest/esm-basic@1.0.0");
  let cjs_default_export_integrity = get_lockfile_npm_package_integrity(
    &lockfile,
    "@denotest/cjs-default-export@1.0.0",
  );

  lockfile.assert_matches_json(json!({
    "version": "5",
    "specifiers": {
      "npm:@denotest/cjs-default-export@1": "1.0.0",
      "npm:@denotest/esm-basic@1": "1.0.0"
    },
    "npm": {
      "@denotest/cjs-default-export@1.0.0": {
        "integrity": cjs_default_export_integrity,
        "tarball": "http://localhost:4260/@denotest/cjs-default-export/1.0.0.tgz"
      },
      "@denotest/esm-basic@1.0.0": {
        "integrity": esm_basic_integrity,
        "tarball": "http://localhost:4260/@denotest/esm-basic/1.0.0.tgz"
      }
    },
    "workspace": {
      "packageJson": {
        "dependencies": [
          "npm:@denotest/cjs-default-export@1"
        ]
      },
      "members": {
        "package-a": {
          "packageJson": {
            "dependencies": [
              "npm:@denotest/esm-basic@1"
            ]
          }
        }
      }
    }
  }));

  // run a command that causes discovery of the root package.json beside the lockfile
  context
    .new_command()
    .args("run main.ts")
    .run()
    .skip_output_check();
  // now we should see the dependencies
  let cjs_default_export_integrity = get_lockfile_npm_package_integrity(
    &lockfile,
    "@denotest/cjs-default-export@1.0.0",
  );
  let expected_lockfile = json!({
    "version": "5",
    "specifiers": {
      "npm:@denotest/cjs-default-export@1": "1.0.0",
      "npm:@denotest/esm-basic@1": "1.0.0"
    },
    "npm": {
      "@denotest/cjs-default-export@1.0.0": {
        "integrity": cjs_default_export_integrity,
        "tarball": "http://localhost:4260/@denotest/cjs-default-export/1.0.0.tgz"
      },
      "@denotest/esm-basic@1.0.0": {
        "integrity": esm_basic_integrity,
        "tarball": "http://localhost:4260/@denotest/esm-basic/1.0.0.tgz"
      }
    },
    "workspace": {
      "packageJson": {
        "dependencies": [
          "npm:@denotest/cjs-default-export@1"
        ]
      },
      "members": {
        "package-a": {
          "packageJson": {
            "dependencies": [
              "npm:@denotest/esm-basic@1"
            ]
          }
        }
      }
    }
  });
  lockfile.assert_matches_json(expected_lockfile.clone());

  // now run the command again in the package with the nested package.json
  context
    .new_command()
    .args("run package-a/main.ts")
    .run()
    .skip_output_check();
  // the lockfile should stay the same as the above because the package.json
  // was found in a different directory
  lockfile.assert_matches_json(expected_lockfile.clone());
}

fn get_lockfile_npm_package_integrity(
  lockfile: &PathRef,
  package_name: &str,
) -> String {
  // todo(dsherret): it would be nice if the test server didn't produce
  // different hashes depending on what operating system it's running on
  lockfile
    .read_json_value()
    .get("npm")
    .unwrap()
    .get(package_name)
    .unwrap()
    .get("integrity")
    .unwrap()
    .as_str()
    .unwrap()
    .to_string()
}

itest!(error_013_missing_script {
  args: "run --reload missing_file_name",
  exit_code: 1,
  output: "run/error_013_missing_script.out",
});

// We have an allow-import flag but not allow-read, it should still result in error.
itest!(error_016_dynamic_import_permissions2 {
  args:
    "run --reload --allow-import run/error_016_dynamic_import_permissions2.js",
  output: "run/error_016_dynamic_import_permissions2.out",
  exit_code: 1,
  http_server: true,
});

itest!(error_026_remote_import_error {
  args: "run --allow-import run/error_026_remote_import_error.ts",
  output: "run/error_026_remote_import_error.ts.out",
  exit_code: 1,
  http_server: true,
});

itest!(error_local_static_import_from_remote_ts {
  args: "run --allow-import --reload http://localhost:4545/run/error_local_static_import_from_remote.ts",
  exit_code: 1,
  http_server: true,
  output: "run/error_local_static_import_from_remote.ts.out",
});

itest!(error_local_static_import_from_remote_js {
  args: "run --allow-import --reload http://localhost:4545/run/error_local_static_import_from_remote.js",
  exit_code: 1,
  http_server: true,
  output: "run/error_local_static_import_from_remote.js.out",
});

itest!(import_meta {
  args: "run --allow-import --quiet --reload --import-map=run/import_meta/importmap.json run/import_meta/main.ts",
  output: "run/import_meta/main.out",
  http_server: true,
});

itest!(no_check_remote {
  args: "run --allow-import --quiet --reload --no-check=remote run/no_check_remote.ts",
  output: "run/no_check_remote.ts.enabled.out",
  http_server: true,
});

#[test]
fn type_directives_js_main() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("run --reload -L debug --check run/type_directives_js_main.js")
    .run();
  output.assert_matches_text("[WILDCARD] - FileFetcher::fetch_no_follow - specifier: file:///[WILDCARD]/subdir/type_reference.d.ts[WILDCARD]");
  let output = context
    .new_command()
    .args("run --reload -L debug run/type_directives_js_main.js")
    .run();
  assert_not_contains!(output.combined_output(), "type_reference.d.ts");
}

itest!(type_directives_redirect {
  args: "run --allow-import --reload --check run/type_directives_redirect.ts",
  output: "run/type_directives_redirect.ts.out",
  http_server: true,
});

itest!(disallow_http_from_https_js {
  args: "run --allow-import --quiet --reload --cert tls/RootCA.pem https://localhost:5545/run/disallow_http_from_https.js",
  output: "run/disallow_http_from_https_js.out",
  http_server: true,
  exit_code: 1,
});

itest!(disallow_http_from_https_ts {
  args: "run --allow-import --quiet --reload --cert tls/RootCA.pem https://localhost:5545/run/disallow_http_from_https.ts",
  output: "run/disallow_http_from_https_ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(jsx_import_source_import_map_scoped {
  args: "run --allow-import --reload --import-map jsx/import-map-scoped.json --no-lock --config jsx/deno-jsx-import-map.jsonc subdir/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_import_map.out",
  http_server: true,
});

itest!(jsx_import_source_import_map_scoped_dev {
  args: "run --allow-import --reload --import-map jsx/import-map-scoped.json --no-lock --config jsx/deno-jsxdev-import-map.jsonc subdir/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_import_map_dev.out",
  http_server: true,
});

// FIXME(bartlomieju): disabled, because this test is very flaky on CI
// itest!(local_sources_not_cached_in_memory {
//   args: "run --allow-read --allow-write run/no_mem_cache.js",
//   output: "run/no_mem_cache.js.out",
// });

#[test]
fn no_validate_asm() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("run/no_validate_asm.js")
    .piped_output()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(output.stderr.is_empty());
  assert!(output.stdout.is_empty());
}

#[test]
fn exec_path() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("run/exec_path.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  let actual = PathRef::new(std::path::Path::new(stdout_str)).canonicalize();
  let expected = util::deno_exe_path().canonicalize();
  assert_eq!(expected, actual);
}

#[test]
fn run_from_stdin_defaults_to_ts() {
  let source_code = r#"
interface Lollipop {
  _: number;
}
console.log("executing typescript");
"#;

  let mut p = util::deno_cmd()
    .arg("run")
    .arg("--check")
    .arg("-")
    .stdin(std::process::Stdio::piped())
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdin = p.stdin.as_mut().unwrap();
  stdin.write_all(source_code.as_bytes()).unwrap();
  let result = p.wait_with_output().unwrap();
  assert!(result.status.success());
  let stdout_str = std::str::from_utf8(&result.stdout).unwrap().trim();
  assert_eq!(stdout_str, "executing typescript");
}

#[test]
fn run_from_stdin_ext() {
  let source_code = r#"
let i = 123;
i = "hello"
console.log("executing javascript");
"#;

  let mut p = util::deno_cmd()
    .args("run --ext js --check -")
    .stdin(std::process::Stdio::piped())
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdin = p.stdin.as_mut().unwrap();
  stdin.write_all(source_code.as_bytes()).unwrap();
  let result = p.wait_with_output().unwrap();
  assert!(result.status.success());
  let stdout_str = std::str::from_utf8(&result.stdout).unwrap().trim();
  assert_eq!(stdout_str, "executing javascript");
}

#[cfg(windows)]
// Clippy suggests to remove the `NoStd` prefix from all variants. I disagree.
#[allow(clippy::enum_variant_names)]
enum WinProcConstraints {
  NoStdIn,
  NoStdOut,
  NoStdErr,
}

#[cfg(windows)]
fn run_deno_script_constrained(
  script_path: test_util::PathRef,
  constraints: WinProcConstraints,
) -> Result<(), i64> {
  let file_path = "assets/DenoWinRunner.ps1";
  let constraints = match constraints {
    WinProcConstraints::NoStdIn => "1",
    WinProcConstraints::NoStdOut => "2",
    WinProcConstraints::NoStdErr => "4",
  };
  let deno_exe_path = util::deno_exe_path().to_string();
  let deno_script_path = script_path.to_string();
  let args = vec![&deno_exe_path[..], &deno_script_path[..], constraints];
  util::run_powershell_script_file(file_path, args)
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stdin() {
  let output = run_deno_script_constrained(
    util::testdata_path().join("echo.ts"),
    WinProcConstraints::NoStdIn,
  );
  output.unwrap();
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stdout() {
  let output = run_deno_script_constrained(
    util::testdata_path().join("echo.ts"),
    WinProcConstraints::NoStdOut,
  );
  output.unwrap();
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stderr() {
  let output = run_deno_script_constrained(
    util::testdata_path().join("echo.ts"),
    WinProcConstraints::NoStdErr,
  );
  output.unwrap();
}

#[cfg(not(windows))]
#[test]
fn should_not_panic_on_undefined_home_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("echo.ts")
    .env_remove("HOME")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[test]
fn should_not_panic_on_undefined_deno_dir_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("echo.ts")
    .env_remove("DENO_DIR")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[cfg(not(windows))]
#[test]
fn should_not_panic_on_undefined_deno_dir_and_home_environment_variables() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("echo.ts")
    .env_remove("DENO_DIR")
    .env_remove("HOME")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[test]
fn deno_log() {
  // Without DENO_LOG the stderr is empty.
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("run/001_hello.js")
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(output.stderr.is_empty());

  // With DENO_LOG the stderr is not empty.
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("run/001_hello.js")
    .env("DENO_LOG", "debug")
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(!output.stderr.is_empty());
}

#[test]
fn dont_cache_on_check_fail() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("run --check=all --reload run/error_003_typescript.ts")
    .split_output()
    .run();
  assert!(!output.stderr().is_empty());
  output.skip_stdout_check();
  output.assert_exit_code(1);

  let output = context
    .new_command()
    .args("run --check=all run/error_003_typescript.ts")
    .split_output()
    .run();
  assert!(!output.stderr().is_empty());
  output.skip_stdout_check();
  output.assert_exit_code(1);
}

mod permissions {
  use test_util as util;
  use util::TestContext;

  #[test]
  fn with_allow() {
    for permission in &util::PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(util::testdata_path())
        .arg("run")
        .arg(format!("--allow-{permission}"))
        .arg("run/permission_test.ts")
        .arg(format!("{permission}Required"))
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn without_allow() {
    for permission in &util::PERMISSION_VARIANTS {
      let (_, err) = util::run_and_collect_output(
        false,
        &format!("run run/permission_test.ts {permission}Required"),
        None,
        None,
        false,
      );
      assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
    }
  }

  #[test]
  fn rw_inside_project_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(util::testdata_path())
        .arg("run")
        .arg(format!(
          "--allow-{0}={1}",
          permission,
          util::testdata_path()
        ))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn rw_outside_test_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let (_, err) = util::run_and_collect_output(
        false,
        &format!(
          "run --allow-{0}={1} run/complex_permissions_test.ts {0} {2}",
          permission,
          util::testdata_path(),
          util::root_path().join("Cargo.toml"),
        ),
        None,
        None,
        false,
      );
      assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
    }
  }

  #[test]
  fn rw_inside_test_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(util::testdata_path())
        .arg("run")
        .arg(format!(
          "--allow-{0}={1}",
          permission,
          util::testdata_path(),
        ))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn rw_outside_test_and_js_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    let test_dir = util::testdata_path();
    let js_dir = util::root_path().join("js");
    for permission in &PERMISSION_VARIANTS {
      let (_, err) = util::run_and_collect_output(
        false,
        &format!(
          "run --allow-{0}={1},{2} run/complex_permissions_test.ts {0} {3}",
          permission,
          test_dir,
          js_dir,
          util::root_path().join("Cargo.toml"),
        ),
        None,
        None,
        false,
      );
      assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
    }
  }

  #[test]
  fn rw_inside_test_and_js_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    let test_dir = util::testdata_path();
    let js_dir = util::root_path().join("js");
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(util::testdata_path())
        .arg("run")
        .arg(format!("--allow-{permission}={test_dir},{js_dir}"))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn rw_relative() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(util::testdata_path())
        .arg("run")
        .arg(format!("--allow-{permission}=."))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn rw_no_prefix() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(util::testdata_path())
        .arg("run")
        .arg(format!("--allow-{permission}=tls/../"))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn net_fetch_allow_localhost_4545() {
    // ensure the http server is running for those tests so they run
    // deterministically whether the http server is running or not
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost:4545 run/complex_permissions_test.ts netFetch http://localhost:4545/",
        None,
        None,
        true,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_fetch_allow_deno_land() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=deno.land run/complex_permissions_test.ts netFetch http://localhost:4545/",
        None,
        None,
        true,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_fetch_localhost_4545_fail() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=localhost:4545 run/complex_permissions_test.ts netFetch http://localhost:4546/",
        None,
        None,
        true,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_fetch_localhost() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost run/complex_permissions_test.ts netFetch http://localhost:4545/ http://localhost:4546/ http://localhost:4547/",
        None,
        None,
        true,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_connect_allow_localhost_ip_4555() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=127.0.0.1:4545 run/complex_permissions_test.ts netConnect 127.0.0.1:4545",
        None,
        None,
        true,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_connect_allow_deno_land() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=deno.land run/complex_permissions_test.ts netConnect 127.0.0.1:4546",
        None,
        None,
        true,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_connect_allow_localhost_ip_4545_fail() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=127.0.0.1:4545 run/complex_permissions_test.ts netConnect 127.0.0.1:4546",
        None,
        None,
        true,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_connect_allow_localhost_ip() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=127.0.0.1 run/complex_permissions_test.ts netConnect 127.0.0.1:4545 127.0.0.1:4546 127.0.0.1:4547",
        None,
        None,
        true,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_localhost_4555() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost:4588 run/complex_permissions_test.ts netListen localhost:4588",
        None,
        None,
        false,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_deno_land() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=deno.land run/complex_permissions_test.ts netListen localhost:4545",
        None,
        None,
        false,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_localhost_4555_fail() {
    let _http_guard = util::http_server();
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=localhost:4555 run/complex_permissions_test.ts netListen localhost:4556",
        None,
        None,
        false,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_localhost() {
    let _http_guard = util::http_server();
    // Port 4600 is chosen to not collide with those used by
    // target/debug/test_server
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost run/complex_permissions_test.ts netListen localhost:4600",
        None,
        None,
        false,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn _061_permissions_request() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/061_permissions_request.ts"])
      .with_pty(|mut console| {
        console.expect(concat!(
          "┏ ⚠️  Deno requests read access to \"foo\".\r\n",
          "┠─ Requested by `Deno.permissions.request()` API.\r\n",
          "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
          "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
          "┠─ Run again with --allow-read to bypass this prompt.\r\n",
          "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.human_delay();
        console.write_line_raw("y");
        console.expect(concat!(
          "┏ ⚠️  Deno requests read access to \"bar\".\r\n",
          "┠─ Requested by `Deno.permissions.request()` API.\r\n",
          "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
          "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
          "┠─ Run again with --allow-read to bypass this prompt.\r\n",
          "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.human_delay();
        console.write_line_raw("n");
        console.expect("granted");
        console.expect("prompt");
        console.expect("denied");
      });
  }

  #[test]
  fn _061_permissions_request_sync() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/061_permissions_request_sync.ts"])
      .with_pty(|mut console| {
        console.expect(concat!(
          "┏ ⚠️  Deno requests read access to \"foo\".\r\n",
          "┠─ Requested by `Deno.permissions.request()` API.\r\n",
          "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
          "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
          "┠─ Run again with --allow-read to bypass this prompt.\r\n",
          "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.human_delay();
        console.write_line_raw("y");
        console.expect(concat!(
          "┏ ⚠️  Deno requests read access to \"bar\".\r\n",
          "┠─ Requested by `Deno.permissions.request()` API.\r\n",
          "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
          "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
          "┠─ Run again with --allow-read to bypass this prompt.\r\n",
          "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.human_delay();
        console.write_line_raw("n");
        console.expect("granted");
        console.expect("prompt");
        console.expect("denied");
      });
  }

  #[test]
  fn _062_permissions_request_global() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/062_permissions_request_global.ts"])
      .with_pty(|mut console| {
        console.expect(concat!(
          "┏ ⚠️  Deno requests read access.\r\n",
          "┠─ Requested by `Deno.permissions.request()` API.\r\n",
          "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
          "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
          "┠─ Run again with --allow-read to bypass this prompt.\r\n",
          "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.human_delay();
        console.write_line_raw("y\n");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
      });
  }

  #[test]
  fn _062_permissions_request_global_sync() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/062_permissions_request_global_sync.ts"])
      .with_pty(|mut console| {
        console.expect(concat!(
          "┏ ⚠️  Deno requests read access.\r\n",
          "┠─ Requested by `Deno.permissions.request()` API.\r\n",
          "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
          "┠─ Learn more at: https://docs.deno.com/go/--allow-read\r\n",
          "┠─ Run again with --allow-read to bypass this prompt.\r\n",
          "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.human_delay();
        console.write_line_raw("y");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
      });
  }

  #[flaky_test::flaky_test]
  fn _066_prompt() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/066_prompt.ts"])
      .with_pty(|mut console| {
        console.expect("What is your name? Jane Doe");
        console.write_line_raw("");
        console.expect("Your name is Jane Doe.");

        console.expect("Prompt ");
        console.write_line_raw("foo");
        console.expect("Your input is foo.");
        console.expect("Question 0 [y/N] ");
        console.write_line_raw("Y");
        console.expect("Your answer is true");
        console.expect("Question 1 [y/N] ");
        console.write_line_raw("N");
        console.expect("Your answer is false");
        console.expect("Question 2 [y/N] ");
        console.write_line_raw("yes");
        console.expect("Your answer is false");
        console.expect("Confirm [y/N] ");
        console.write_line("");
        console.expect("Your answer is false");
        console.expect("What is Windows EOL? ");
        console.write_line("windows");
        console.expect("Your answer is \"windows\"");
        console.expect("Hi [Enter] ");
        console.write_line("");
        console.expect("Alert [Enter] ");
        console.write_line("");
        console.expect("The end of test");
      });
  }
}

#[flaky_test::flaky_test]
#[cfg(windows)]
fn process_stdin_read_unblock() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "run/process_stdin_unblock.mjs"])
    .with_pty(|mut console| {
      console.write_raw("b");
      console.human_delay();
      console.write_line_raw("s");
      console.expect_all(&["1", "1"]);
    });
}

#[test]
fn issue9750() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "run/issue9750.js"])
    .with_pty(|mut console| {
      console.expect("Enter 'yy':");
      console.write_line_raw("yy");
      console.expect(concat!(
        "┏ ⚠️  Deno requests env access.\r\n",
        "┠─ Requested by `Deno.permissions.request()` API.\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-env\r\n",
        "┠─ Run again with --allow-env to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all env permissions)",
      ));
      console.human_delay();
      console.write_line_raw("n");
      console.expect("Denied env access.");
      console.expect(concat!(
        "┏ ⚠️  Deno requests env access to \"SECRET\".\r\n",
        "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.\r\n",
        "┠─ Learn more at: https://docs.deno.com/go/--allow-env\r\n",
        "┠─ Run again with --allow-env to bypass this prompt.\r\n",
        "┗ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all env permissions)",
      ));
      console.human_delay();
      console.write_line_raw("n");
      console.expect_all(&[
        "Denied env access to \"SECRET\".",
        "NotCapable: Requires env access to \"SECRET\", run again with the --allow-env flag",
      ]);
    });
}

#[test]
#[cfg(unix)]
fn navigator_language_unix() {
  let (res, _) = util::run_and_collect_output(
    true,
    "run navigator_language.ts",
    None,
    Some(vec![("LC_ALL".to_owned(), "pl_PL".to_owned())]),
    false,
  );
  assert_eq!(res, "pl-PL\n")
}

#[test]
fn navigator_language() {
  let (res, _) = util::run_and_collect_output(
    true,
    "run navigator_language.ts",
    None,
    None,
    false,
  );
  assert!(!res.is_empty())
}

#[test]
#[cfg(unix)]
fn navigator_languages_unix() {
  let (res, _) = util::run_and_collect_output(
    true,
    "run navigator_languages.ts",
    None,
    Some(vec![
      ("LC_ALL".to_owned(), "pl_PL".to_owned()),
      ("NO_COLOR".to_owned(), "1".to_owned()),
    ]),
    false,
  );
  assert_eq!(res, "[ \"pl-PL\" ]\n")
}

#[test]
fn navigator_languages() {
  let (res, _) = util::run_and_collect_output(
    true,
    "run navigator_languages.ts",
    None,
    None,
    false,
  );
  assert!(!res.is_empty())
}

/// Regression test for https://github.com/denoland/deno/issues/12740.
#[test]
fn issue12740() {
  let mod_dir = TempDir::new();
  let mod1_path = mod_dir.path().join("mod1.ts");
  let mod2_path = mod_dir.path().join("mod2.ts");
  mod1_path.write("");
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&mod1_path)
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  mod1_path.write("export { foo } from \"./mod2.ts\";");
  mod2_path.write("(");
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&mod1_path)
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(!status.success());
}

/// Regression test for https://github.com/denoland/deno/issues/12807.
#[test]
fn issue12807() {
  let mod_dir = TempDir::new();
  let mod1_path = mod_dir.path().join("mod1.ts");
  let mod2_path = mod_dir.path().join("mod2.ts");
  // With a fresh `DENO_DIR`, run a module with a dependency and a type error.
  mod1_path.write("import './mod2.ts'; Deno.exit('0');");
  mod2_path.write("console.log('Hello, world!');");
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--check")
    .arg(&mod1_path)
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(!status.success());
  // Fix the type error and run again.
  std::fs::write(&mod1_path, "import './mod2.ts'; Deno.exit(0);").unwrap();
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--check")
    .arg(&mod1_path)
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn package_json_no_node_modules_dir_created() {
  // it should not create a node_modules directory
  let context = TestContextBuilder::new()
    .add_npm_env_vars()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();

  temp_dir.write("deno.json", "{}");
  temp_dir.write("package.json", "{}");
  temp_dir.write("main.ts", "");

  context.new_command().args("run main.ts").run();

  assert!(!temp_dir.path().join("node_modules").exists());
}

#[test]
fn node_modules_dir_no_npm_specifiers_no_dir_created() {
  // it should not create a node_modules directory
  let context = TestContextBuilder::new()
    .add_npm_env_vars()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();

  temp_dir.write("deno.json", "{}");
  temp_dir.write("main.ts", "");

  context
    .new_command()
    .args("run --node-modules-dir main.ts")
    .run();

  assert!(!temp_dir.path().join("node_modules").exists());
}

#[test]
fn check_local_then_remote() {
  let _http_guard = util::http_server();
  let deno_dir = util::new_deno_dir();
  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-import")
    .arg("--check")
    .arg("run/remote_type_error/main.ts")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-import")
    .arg("--check=all")
    .arg("run/remote_type_error/main.ts")
    .env("NO_COLOR", "1")
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  assert_contains!(stderr, "Type 'string' is not assignable to type 'number'.");
}

#[test]
fn permission_request_with_no_prompt() {
  TestContext::default()
    .new_command()
    .env("NO_COLOR", "1")
    .args_vec([
      "run",
      "--quiet",
      "--no-prompt",
      "run/permission_request_no_prompt.ts",
    ])
    .with_pty(|mut console| {
      console.expect("PermissionStatus { state: \"denied\", onchange: null }");
    });
}

#[test]
fn deno_no_prompt_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("run/no_prompt.ts")
    .env("DENO_NO_PROMPT", "1")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[test]
fn running_declaration_files() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let files = vec!["file.d.ts", "file.d.cts", "file.d.mts"];

  for file in files {
    temp_dir.write(file, "");
    context
      .new_command()
      // todo(dsherret): investigate why --allow-read is required here
      .args_vec(["run", "--allow-read", file])
      .run()
      .skip_output_check()
      .assert_exit_code(0);
  }
}

#[cfg(not(target_os = "windows"))]
itest!(spawn_kill_permissions {
  args: "run --quiet --allow-run=cat spawn_kill_permissions.ts",
  envs: vec![
    ("LD_LIBRARY_PATH".to_string(), "".to_string()),
    ("DYLD_FALLBACK_LIBRARY_PATH".to_string(), "".to_string())
  ],
  output_str: Some(""),
});

#[test]
fn cache_test() {
  let _g = util::http_server();
  let deno_dir = TempDir::new();
  let module_url =
    url::Url::parse("http://localhost:4545/run/006_url_imports.ts").unwrap();
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("cache")
    .arg("--allow-import=localhost:4545")
    .arg("--check=all")
    .arg("-L")
    .arg("debug")
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());

  let prg = util::deno_exe_path();
  let output = Command::new(prg)
    .env("DENO_DIR", deno_dir.path())
    .env("HTTP_PROXY", "http://nil")
    .env("NO_COLOR", "1")
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-import=localhost:4545")
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");

  let str_output = std::str::from_utf8(&output.stdout).unwrap();

  let module_output_path =
    util::testdata_path().join("run/006_url_imports.ts.out");
  let mut module_output = String::new();
  let mut module_output_file = std::fs::File::open(module_output_path).unwrap();
  module_output_file
    .read_to_string(&mut module_output)
    .unwrap();

  assert_eq!(module_output, str_output);
}

#[test]
fn cache_invalidation_test() {
  let deno_dir = TempDir::new();
  let fixture_path = deno_dir.path().join("fixture.ts");
  fixture_path.write("console.log(\"42\");");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&fixture_path)
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "42\n");
  fixture_path.write("console.log(\"43\");");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(fixture_path)
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "43\n");
}

#[test]
fn cache_invalidation_test_no_check() {
  let deno_dir = TempDir::new();
  let fixture_path = deno_dir.path().join("fixture.ts");
  fixture_path.write("console.log(\"42\");");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--no-check")
    .arg(&fixture_path)
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "42\n");
  fixture_path.write("console.log(\"43\");");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--no-check")
    .arg(fixture_path)
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "43\n");
}

#[test]
fn ts_dependency_recompilation() {
  let t = TempDir::new();
  let ats = t.path().join("a.ts");

  std::fs::write(
    &ats,
    "
    import { foo } from \"./b.ts\";

    function print(str: string): void {
        console.log(str);
    }

    print(foo);",
  )
  .unwrap();

  let bts = t.path().join("b.ts");
  std::fs::write(
    &bts,
    "
    export const foo = \"foo\";",
  )
  .unwrap();

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--check")
    .arg(&ats)
    .output()
    .expect("failed to spawn script");

  let stdout_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_output = std::str::from_utf8(&output.stderr).unwrap().trim();

  assert!(stdout_output.ends_with("foo"));
  assert!(stderr_output.starts_with("Check"));

  // Overwrite contents of b.ts and run again
  std::fs::write(
    &bts,
    "
    export const foo = 5;",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--check")
    .arg(&ats)
    .output()
    .expect("failed to spawn script");

  let stdout_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_output = std::str::from_utf8(&output.stderr).unwrap().trim();

  // TS2345 [ERROR]: Argument of type '5' is not assignable to parameter of type 'string'.
  assert!(stderr_output.contains("TS2345"));
  assert!(!output.status.success());
  assert!(stdout_output.is_empty());
}

#[test]
fn basic_auth_tokens() {
  let _g = util::http_server();

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("--allow-import")
    .arg("http://127.0.0.1:4554/run/001_hello.js")
    .piped_output()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(!output.status.success());

  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert!(stdout_str.is_empty());

  let stderr_str = std::str::from_utf8(&output.stderr).unwrap().trim();
  eprintln!("{stderr_str}");

  assert!(stderr_str
    .contains("Module not found \"http://127.0.0.1:4554/run/001_hello.js\"."));

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("--allow-import")
    .arg("http://127.0.0.1:4554/run/001_hello.js")
    .env("DENO_AUTH_TOKENS", "testuser123:testpassabc@127.0.0.1:4554")
    .piped_output()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  let stderr_str = std::str::from_utf8(&output.stderr).unwrap().trim();
  eprintln!("{stderr_str}");

  assert!(output.status.success());

  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert_eq!(util::strip_ansi_codes(stdout_str), "Hello World");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_resolve_dns() {
  use std::net::SocketAddr;
  use std::str::FromStr;
  use std::sync::Arc;
  use std::time::Duration;

  use hickory_server::authority::Catalog;
  use hickory_server::authority::ZoneType;
  use hickory_server::proto::rr::Name;
  use hickory_server::store::in_memory::InMemoryAuthority;
  use hickory_server::ServerFuture;
  use tokio::net::TcpListener;
  use tokio::net::UdpSocket;
  use tokio::sync::oneshot;

  const DNS_PORT: u16 = 4553;

  // Setup DNS server for testing
  async fn run_dns_server(tx: oneshot::Sender<()>) {
    let zone_file = std::fs::read_to_string(
      util::testdata_path().join("run/resolve_dns.zone.in"),
    )
    .unwrap();
    let records = Parser::new(
      &zone_file,
      None,
      Some(Name::from_str("example.com").unwrap()),
    )
    .parse();
    if records.is_err() {
      panic!("failed to parse: {:?}", records.err())
    }
    let (origin, records) = records.unwrap();
    let authority: Vec<Arc<dyn AuthorityObject>> = vec![Arc::new(
      InMemoryAuthority::new(origin, records, ZoneType::Primary, false)
        .unwrap(),
    )];
    let mut catalog: Catalog = Catalog::new();
    catalog.upsert(Name::root().into(), authority);

    let mut server_fut = ServerFuture::new(catalog);
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], DNS_PORT));
    let tcp_listener = TcpListener::bind(socket_addr).await.unwrap();
    let udp_socket = UdpSocket::bind(socket_addr).await.unwrap();
    server_fut.register_socket(udp_socket);
    server_fut.register_listener(tcp_listener, Duration::from_secs(2));

    // Notifies that the DNS server is ready
    tx.send(()).unwrap();

    server_fut.block_until_done().await.unwrap();
  }

  let (ready_tx, ready_rx) = oneshot::channel();
  let dns_server_fut = run_dns_server(ready_tx);
  let handle = tokio::spawn(dns_server_fut);

  // Waits for the DNS server to be ready
  ready_rx.await.unwrap();

  // Pass: `--allow-net`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--check")
      .arg("--allow-net")
      .arg("run/resolve_dns.ts")
      .piped_output()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    println!("{err}");
    assert!(output.status.success());
    assert!(err.starts_with("Check file"));

    let expected = std::fs::read_to_string(
      util::testdata_path().join("run/resolve_dns.ts.out"),
    )
    .unwrap();
    assert_eq!(expected, out);
  }

  // Pass: `--allow-net=127.0.0.1:4553`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--check")
      .arg("--allow-net=127.0.0.1:4553")
      .arg("run/resolve_dns.ts")
      .piped_output()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
      eprintln!("stderr: {err}");
    }
    assert!(output.status.success());
    assert!(err.starts_with("Check file"));

    let expected = std::fs::read_to_string(
      util::testdata_path().join("run/resolve_dns.ts.out"),
    )
    .unwrap();
    assert_eq!(expected, out);
  }

  // Permission error: `--allow-net=deno.land`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--check")
      .arg("--allow-net=deno.land")
      .arg("run/resolve_dns.ts")
      .piped_output()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(err.starts_with("Check file"));
    assert!(err.contains(r#"error: Uncaught (in promise) NotCapable: Requires net access to "127.0.0.1:4553""#));
    assert!(out.is_empty());
  }

  // Permission error: no permission specified
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--check")
      .arg("run/resolve_dns.ts")
      .piped_output()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(err.starts_with("Check file"));
    assert!(err.contains(r#"error: Uncaught (in promise) NotCapable: Requires net access to "127.0.0.1:4553""#));
    assert!(out.is_empty());
  }

  handle.abort();
}

#[tokio::test]
async fn http2_request_url() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--quiet")
    .arg("--allow-net")
    .arg("--allow-read")
    .arg("./run/http2_request_url.ts")
    .arg("4506")
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 5];
  let read = stdout.read(&mut buffer).unwrap();
  assert_eq!(read, 5);
  let msg = std::str::from_utf8(&buffer).unwrap();
  assert_eq!(msg, "READY");

  let cert = reqwest::Certificate::from_pem(include_bytes!(
    "../testdata/tls/RootCA.crt"
  ))
  .unwrap();

  let client = reqwest::Client::builder()
    .add_root_certificate(cert)
    .http2_prior_knowledge()
    .build()
    .unwrap();

  let res = client.get("http://127.0.0.1:4506").send().await.unwrap();
  assert_eq!(200, res.status());

  let body = res.text().await.unwrap();
  assert_eq!(body, "http://127.0.0.1:4506/");

  child.kill().unwrap();
  child.wait().unwrap();
}

#[cfg(not(windows))]
#[test]
fn set_raw_should_not_panic_on_no_tty() {
  let output = util::deno_cmd()
    .arg("eval")
    .arg("Deno.stdin.setRaw(true)")
    // stdin set to piped so it certainly does not refer to TTY
    .stdin(std::process::Stdio::piped())
    // stderr is piped so we can capture output.
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stderr = std::str::from_utf8(&output.stderr).unwrap().trim();
  assert!(stderr.contains("BadResource"));
}

#[cfg(not(windows))]
#[test]
fn fsfile_set_raw_should_not_panic_on_no_tty() {
  let output = util::deno_cmd()
    .arg("eval")
    .arg("Deno.openSync(\"/dev/stdin\").setRaw(true)")
    // stdin set to piped so it certainly does not refer to TTY
    .stdin(std::process::Stdio::piped())
    // stderr is piped so we can capture output.
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stderr = std::str::from_utf8(&output.stderr).unwrap().trim();
  assert!(
    stderr.contains("BadResource"),
    "stderr did not contain BadResource: {stderr}"
  );
}

#[test]
fn timeout_clear() {
  // https://github.com/denoland/deno/issues/7599

  use std::time::Duration;
  use std::time::Instant;

  let source_code = r#"
const handle = setTimeout(() => {
  console.log("timeout finish");
}, 10000);
clearTimeout(handle);
console.log("finish");
"#;

  let mut p = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("-")
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  let stdin = p.stdin.as_mut().unwrap();
  stdin.write_all(source_code.as_bytes()).unwrap();
  let start = Instant::now();
  let status = p.wait().unwrap();
  let end = Instant::now();
  assert!(status.success());
  // check that program did not run for 10 seconds
  // for timeout to clear
  assert!(end - start < Duration::new(10, 0));
}

#[test]
fn broken_stdout() {
  let (reader, writer) = os_pipe::pipe().unwrap();
  // drop the reader to create a broken pipe
  drop(reader);

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("eval")
    .arg("console.log(3.14)")
    .stdout(writer)
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(!output.status.success());
  let stderr = std::str::from_utf8(output.stderr.as_ref()).unwrap().trim();
  assert!(stderr.contains("Uncaught (in promise) BrokenPipe"));
  assert!(!stderr.contains("panic"));
}

#[test]
fn broken_stdout_repl() {
  let (reader, writer) = os_pipe::pipe().unwrap();
  // drop the reader to create a broken pipe
  drop(reader);

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("repl")
    .stdout(writer)
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(!output.status.success());
  let stderr = std::str::from_utf8(output.stderr.as_ref()).unwrap().trim();
  if cfg!(windows) {
    assert_contains!(stderr, "The pipe is being closed. (os error 232)");
  } else {
    assert_contains!(stderr, "Broken pipe (os error 32)");
  }
  assert_not_contains!(stderr, "panic");
}

#[tokio::test(flavor = "multi_thread")]
async fn websocketstream_ping() {
  let _g = util::http_server();

  let script = util::testdata_path().join("run/websocketstream_ping_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");

  let srv_fn = hyper::service::service_fn(|mut req| async move {
    let (response, upgrade_fut) =
      fastwebsockets::upgrade::upgrade(&mut req).unwrap();
    tokio::spawn(async move {
      let mut ws = upgrade_fut.await.unwrap();

      ws.write_frame(fastwebsockets::Frame::text(b"A"[..].into()))
        .await
        .unwrap();
      ws.write_frame(fastwebsockets::Frame::new(
        true,
        fastwebsockets::OpCode::Ping,
        None,
        vec![].into(),
      ))
      .await
      .unwrap();
      ws.write_frame(fastwebsockets::Frame::text(b"B"[..].into()))
        .await
        .unwrap();
      let message = ws.read_frame().await.unwrap();
      assert_eq!(message.opcode, fastwebsockets::OpCode::Pong);
      ws.write_frame(fastwebsockets::Frame::text(b"C"[..].into()))
        .await
        .unwrap();
      ws.write_frame(fastwebsockets::Frame::close_raw(vec![].into()))
        .await
        .unwrap();
    });
    Ok::<_, std::convert::Infallible>(response)
  });

  let child = util::deno_cmd()
    .arg("test")
    .arg("--unstable-net")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .stdout_piped()
    .spawn()
    .unwrap();
  let server = tokio::net::TcpListener::bind("127.0.0.1:4513")
    .await
    .unwrap();
  tokio::spawn(async move {
    let (stream, _) = server.accept().await.unwrap();
    let io = hyper_util::rt::TokioIo::new(stream);
    let conn_fut = hyper::server::conn::http1::Builder::new()
      .serve_connection(io, srv_fn)
      .with_upgrades();

    if let Err(e) = conn_fut.await {
      eprintln!("websocket server error: {e:?}");
    }
  });

  let r = child.wait_with_output().unwrap();
  assert!(r.status.success());
}

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
  Fut: std::future::Future + Send + 'static,
  Fut::Output: Send + 'static,
{
  fn execute(&self, fut: Fut) {
    deno_unsync::spawn(fut);
  }
}

#[tokio::test]
async fn websocket_server_multi_field_connection_header() {
  let script = util::testdata_path()
    .join("run/websocket_server_multi_field_connection_header_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .stdout_piped()
    .spawn()
    .unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 5];
  let read = stdout.read(&mut buffer).unwrap();
  assert_eq!(read, 5);
  let msg = std::str::from_utf8(&buffer).unwrap();
  assert_eq!(msg, "READY");

  let stream = tokio::net::TcpStream::connect("localhost:4319")
    .await
    .unwrap();
  let req = http::Request::builder()
    .header(http::header::UPGRADE, "websocket")
    .header(http::header::CONNECTION, "keep-alive, Upgrade")
    .header(
      "Sec-WebSocket-Key",
      fastwebsockets::handshake::generate_key(),
    )
    .header("Sec-WebSocket-Version", "13")
    .uri("ws://localhost:4319")
    .body(http_body_util::Empty::<Bytes>::new())
    .unwrap();

  let (mut socket, _) =
    fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
      .await
      .unwrap();

  let message = socket.read_frame().await.unwrap();
  assert_eq!(message.opcode, fastwebsockets::OpCode::Close);
  socket
    .write_frame(fastwebsockets::Frame::close_raw(vec![].into()))
    .await
    .unwrap();
  assert!(child.wait().unwrap().success());
}

#[tokio::test]
async fn websocket_server_idletimeout() {
  test_util::timeout!(60);
  let script =
    util::testdata_path().join("run/websocket_server_idletimeout.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg("--config")
    .arg("./config/deno.json")
    .arg(script)
    .stdout_piped()
    .spawn()
    .unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut buf: Vec<u8> = vec![];
  while !String::from_utf8(buf.clone()).unwrap().contains("READY") {
    let mut buffer = [0; 64];
    let read = stdout.read(&mut buffer).unwrap();
    buf.extend_from_slice(&buffer[0..read]);
    eprintln!("buf = {buf:?}");
  }

  let stream = tokio::net::TcpStream::connect("localhost:4509")
    .await
    .unwrap();
  let req = http::Request::builder()
    .header(http::header::UPGRADE, "websocket")
    .header(http::header::CONNECTION, "keep-alive, Upgrade")
    .header(
      "Sec-WebSocket-Key",
      fastwebsockets::handshake::generate_key(),
    )
    .header("Sec-WebSocket-Version", "13")
    .uri("ws://localhost:4509")
    .body(http_body_util::Empty::<Bytes>::new())
    .unwrap();

  let (_socket, _) =
    fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
      .await
      .unwrap();
  assert_eq!(child.wait().unwrap().code(), Some(123));
}

// Regression test for https://github.com/denoland/deno/issues/16772
#[test]
fn file_fetcher_preserves_permissions() {
  let context = TestContext::with_http_server();
  context
    .new_command()
    .args("repl --quiet")
    .with_pty(|mut console| {
      console.write_line(
        "const a = await import('http://localhost:4545/run/019_media_types.ts');",
      );
      console.expect("Allow?");
      console.human_delay();
      console.write_line_raw("y");
      console.expect_all(&["success", "true"]);
    });
}

#[test]
fn stdio_streams_are_locked_in_permission_prompt() {
  if !util::pty::Pty::is_supported() {
    // Don't deal with the logic below if the with_pty
    // block doesn't even run (ex. on Windows CI)
    return;
  }

  let context = TestContextBuilder::new().build();

  context
    .new_command()
    .args("repl")
    .with_pty(|mut console| {
      let malicious_output = r#"**malicious**"#;

      // Start a worker that starts spamming stdout
      console.write_line(r#"new Worker(URL.createObjectURL(new Blob(["setInterval(() => console.log('**malicious**'), 10)"])), { type: "module" });"#);
      // The worker is now spamming
      console.expect(malicious_output);
      console.write_line(r#"Deno.readTextFileSync('../Cargo.toml');"#);
      // We will get a permission prompt
      console.expect("Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions) > ");
      // The worker is blocked, so nothing else should get written here
      console.human_delay();
      console.write_line_raw("i");
      // We ensure that nothing gets written here between the permission prompt and this text, despite the delay
      let newline = if cfg!(target_os = "linux") {
        "^J"
      } else {
        "\r\n"
      };
      if cfg!(windows) {
        // it's too difficult to inspect the raw text on windows because the console
        // outputs a bunch of control characters, so we instead rely on the last assertion
        // in this test that checks to ensure we didn't receive any malicious output during
        // the permission prompts
        console.expect("Unrecognized option. Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions) >");
        console.human_delay();
        console.write_line_raw("y");
        console.expect("Granted read access to");
      } else {
        console.expect_raw_next(format!("i{newline}\u{1b}[1A\u{1b}[0J┗ Unrecognized option. Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions) > "));
        console.human_delay();
        console.write_line_raw("y");
        // We ensure that nothing gets written here between the permission prompt and this text, despite the delay
        console.expect_raw_next(format!("y{newline}\x1b[6A\x1b[0J✅ Granted read access to \""));
      }

      // Back to spamming!
      console.expect(malicious_output);

      // Ensure during the permission prompt showing we didn't receive any malicious output
      let all_text = console.all_output();
      let start_prompt_index = all_text.find("Allow?").unwrap();
      let end_prompt_index = all_text.find("Granted read access to").unwrap();
      let prompt_text = &all_text[start_prompt_index..end_prompt_index];
      assert!(!prompt_text.contains(malicious_output), "Prompt text: {:?}", prompt_text);
  });
}

#[test]
fn permission_prompt_escapes_ansi_codes_and_control_chars() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line(
        r#"Deno.permissions.request({ name: "env", variable: "\rDo you like ice cream? y/n" });"#
      );
    // will be uppercase on windows
    let env_name = if cfg!(windows) {
      "\\rDO YOU LIKE ICE CREAM? Y/N"
    } else {
      "\\rDo you like ice cream? y/n"
    };
    console.expect(format!(
      "\u{250f} \u{26a0}\u{fe0f}  Deno requests env access to \"{}\".",
      env_name
    ))
  });

  // windows doesn't support backslashes in paths, so just try this on unix
  if cfg!(unix) {
    let context = TestContextBuilder::default().use_temp_cwd().build();
    context
      .new_command()
      .env("PATH", context.temp_dir().path())
      .env("DYLD_FALLBACK_LIBRARY_PATH", "")
      .env("LD_LIBRARY_PATH", "")
      .args_vec(["repl", "--allow-write=."])
      .with_pty(|mut console| {
        console.write_line_raw(r#"const boldANSI = "\u001b[1m";"#);
        console.expect("undefined");
        console.write_line_raw(r#"const unboldANSI = "\u001b[22m";"#);
        console.expect("undefined");
        console.write_line_raw(
          r#"Deno.writeTextFileSync(`${boldANSI}cat${unboldANSI}`, "");"#,
        );
        console.expect("undefined");
        console.write_line_raw(
          r#"new Deno.Command(`./${boldANSI}cat${unboldANSI}`).spawn();"#,
        );
        console
          .expect("\u{250f} \u{26a0}\u{fe0f}  Deno requests run access to \"");
        console.expect("\\u{1b}[1mcat\\u{1b}[22m\"."); // ensure escaped
      });
  }
}

itest!(extension_import {
  args: "run run/extension_import.ts",
  output: "run/extension_import.ts.out",
  exit_code: 1,
});

itest!(extension_dynamic_import {
  args: "run run/extension_dynamic_import.ts",
  output: "run/extension_dynamic_import.ts.out",
  exit_code: 1,
});

#[test]
pub fn vendor_dir_config_file() {
  let test_context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = test_context.temp_dir();
  let vendor_dir = temp_dir.path().join("vendor");
  let rm_vendor_dir = || std::fs::remove_dir_all(&vendor_dir).unwrap();

  temp_dir.write("deno.json", r#"{ "vendor": true }"#);
  temp_dir.write(
    "main.ts",
    r#"import { returnsHi } from 'http://localhost:4545/subdir/mod1.ts';
console.log(returnsHi());"#,
  );

  let deno_run_cmd = test_context
    .new_command()
    .args("run --allow-import --quiet main.ts");
  deno_run_cmd.run().assert_matches_text("Hi\n");

  assert!(vendor_dir.exists());
  rm_vendor_dir();
  temp_dir.write("deno.json", r#"{ "vendor": false }"#);

  deno_run_cmd.run().assert_matches_text("Hi\n");
  assert!(!vendor_dir.exists());
  test_context
    .new_command()
    .args("cache --allow-import --quiet --vendor main.ts")
    .run();
  assert!(vendor_dir.exists());
  rm_vendor_dir();

  temp_dir.write("deno.json", r#"{ "vendor": true }"#);
  let cache_command = test_context
    .new_command()
    .args("cache --allow-import --quiet main.ts");
  cache_command.run();

  assert!(vendor_dir.exists());
  let mod1_file = vendor_dir
    .join("http_localhost_4545")
    .join("subdir")
    .join("mod1.ts");
  mod1_file.write("export function returnsHi() { return 'bye bye bye'; }");

  // this is fine with a lockfile because users are supposed to be able
  // to modify the vendor folder
  deno_run_cmd.run().assert_matches_text("bye bye bye\n");

  // try updating by deleting the lockfile
  let lockfile = temp_dir.path().join("deno.lock");
  lockfile.remove_file();
  cache_command.run();

  // should still run and the lockfile should be recreated
  // (though with the checksum from the vendor folder)
  deno_run_cmd.run().assert_matches_text("bye bye bye\n");
  assert!(lockfile.exists());

  // ensure we can add and execute files in directories that have a hash in them
  test_context
    .new_command()
    // http_localhost_4545/subdir/#capitals_c75d7/main.js
    .args("cache --allow-import http://localhost:4545/subdir/CAPITALS/main.js")
    .run()
    .skip_output_check();
  assert_eq!(
    vendor_dir.join("manifest.json").read_json_value(),
    json!({
      "folders": {
        "http://localhost:4545/subdir/CAPITALS/": "http_localhost_4545/subdir/#capitals_c75d7"
      }
    })
  );
  vendor_dir
    .join("http_localhost_4545/subdir/#capitals_c75d7/hello_there.ts")
    .write("console.log('hello there');");
  test_context
    .new_command()
    // todo(dsherret): seems wrong that we don't auto-discover the config file to get the vendor directory for this
    .args("run --allow-import --vendor http://localhost:4545/subdir/CAPITALS/hello_there.ts")
    .run()
    .assert_matches_text("hello there\n");

  // now try importing directly from the vendor folder
  temp_dir.write(
    "main.ts",
    r#"import { returnsHi } from './vendor/http_localhost_4545/subdir/mod1.ts';
console.log(returnsHi());"#,
  );
  deno_run_cmd
    .run()
    .assert_matches_text("error: Importing from the vendor directory is not permitted. Use a remote specifier instead or disable vendoring.
    at [WILDCARD]/main.ts:1:27
")
    .assert_exit_code(1);
}

#[test]
fn deno_json_imports_expand() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.write(
    "deno.json",
    r#"{
    "imports": {
      "basic": "npm:@denotest/esm-basic"
    }
}"#,
  );

  dir.write(
    "main.ts",
    r#"
// import map should resolve
import { setValue, getValue } from "basic";
// this entry should have been added automatically
import { hello } from "basic/other.mjs";

setValue(5);
console.log(getValue());
console.log(hello());
"#,
  );
  let output = test_context.new_command().args("run main.ts").run();
  output.assert_matches_text("[WILDCARD]5\nhello, world!\n");
}

#[test]
fn deno_json_imports_expand_doesnt_overwrite_existing_entries() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.write(
    "deno.json",
    r#"{
    "imports": {
      "basic": "npm:@denotest/esm-basic",
      "basic/": "npm:/@denotest/sub-folders/folder_index_js/"
    }
}"#,
  );

  dir.write(
    "main.ts",
    r#"
// import map should resolve
import { setValue, getValue } from "basic";
// this entry should map to explicitly specified "basic/" mapping
import { add } from "basic/index.js";

setValue(5);
console.log(getValue());
console.log(add(3, 4));
"#,
  );
  let output = test_context.new_command().args("run main.ts").run();
  output.assert_matches_text("[WILDCARD]5\n7\n");
}

#[test]
fn code_cache_test() {
  let test_context = TestContextBuilder::new().use_temp_cwd().build();
  let deno_dir = test_context.deno_dir();
  let temp_dir = test_context.temp_dir();
  temp_dir.write("main.js", "console.log('Hello World - A');");

  // First run with no prior cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug main.js")
      .split_output()
      .run();

    output
      .assert_stdout_matches_text("Hello World - A[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]Updating V8 code cache for ES module: file:///[WILDCARD]/main.js[WILDCARD]");
    assert_not_contains!(output.stderr(), "V8 code cache hit");

    // Check that the code cache database exists.
    let code_cache_path = deno_dir.path().join(CODE_CACHE_DB_FILE_NAME);
    assert!(code_cache_path.exists());
  }

  // 2nd run with cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug main.js")
      .split_output()
      .run();

    output
      .assert_stdout_matches_text("Hello World - A[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]V8 code cache hit for ES module: file:///[WILDCARD]/main.js[WILDCARD]");
    assert_not_contains!(output.stderr(), "Updating V8 code cache");
  }

  // Rerun with --no-code-cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug --no-code-cache main.js")
      .split_output()
      .run();

    output
      .assert_stdout_matches_text("Hello World - A[WILDCARD]")
      .skip_stderr_check();
    assert_not_contains!(output.stderr(), "V8 code cache");
  }

  // Modify the script, and make sure that the cache is rejected.
  temp_dir.write("main.js", "console.log('Hello World - B');");
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug main.js")
      .split_output()
      .run();

    output
      .assert_stdout_matches_text("Hello World - B[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]Updating V8 code cache for ES module: file:///[WILDCARD]/main.js[WILDCARD]");
    assert_not_contains!(output.stderr(), "V8 code cache hit");
  }
}

#[test]
fn code_cache_npm_test() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let deno_dir = test_context.deno_dir();
  let temp_dir = test_context.temp_dir();
  temp_dir.write(
    "main.js",
    "import chalk from \"npm:chalk@5\";console.log(chalk('Hello World'));",
  );

  // First run with no prior cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug -A main.js")
      .split_output()
      .run();

    output
      .assert_stdout_matches_text("Hello World[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]Updating V8 code cache for ES module: file:///[WILDCARD]/main.js[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]Updating V8 code cache for ES module: file:///[WILDCARD]/chalk/5.[WILDCARD]/source/index.js[WILDCARD]");
    assert_not_contains!(output.stderr(), "V8 code cache hit");

    // Check that the code cache database exists.
    let code_cache_path = deno_dir.path().join(CODE_CACHE_DB_FILE_NAME);
    assert!(code_cache_path.exists());
  }

  // 2nd run with cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug -A main.js")
      .split_output()
      .run();

    output
      .assert_stdout_matches_text("Hello World[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]V8 code cache hit for ES module: file:///[WILDCARD]/main.js[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]V8 code cache hit for ES module: file:///[WILDCARD]/chalk/5.[WILDCARD]/source/index.js[WILDCARD]");
    assert_not_contains!(output.stderr(), "Updating V8 code cache");
  }
}

#[test]
fn code_cache_npm_with_require_test() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let deno_dir = test_context.deno_dir();
  let temp_dir = test_context.temp_dir();
  temp_dir.write(
    "main.js",
    "import fraction from \"npm:autoprefixer\";console.log(typeof fraction);",
  );

  // First run with no prior cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug -A main.js")
      .split_output()
      .run();

    output
      .assert_stdout_matches_text("function[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]Updating V8 code cache for ES module: file:///[WILDCARD]/main.js[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]Updating V8 code cache for ES module: file:///[WILDCARD]/autoprefixer/[WILDCARD]/autoprefixer.js[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]Updating V8 code cache for script: file:///[WILDCARD]/autoprefixer/[WILDCARD]/autoprefixer.js[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]Updating V8 code cache for script: file:///[WILDCARD]/browserslist/[WILDCARD]/index.js[WILDCARD]");
    assert_not_contains!(output.stderr(), "V8 code cache hit");

    // Check that the code cache database exists.
    let code_cache_path = deno_dir.path().join(CODE_CACHE_DB_FILE_NAME);
    assert!(code_cache_path.exists());
  }

  // 2nd run with cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug -A main.js")
      .split_output()
      .run();

    output
      .assert_stdout_matches_text("function[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]V8 code cache hit for ES module: file:///[WILDCARD]/main.js[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]V8 code cache hit for ES module: file:///[WILDCARD]/autoprefixer/[WILDCARD]/autoprefixer.js[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]V8 code cache hit for script: file:///[WILDCARD]/autoprefixer/[WILDCARD]/autoprefixer.js[WILDCARD]")
      .assert_stderr_matches_text("[WILDCARD]V8 code cache hit for script: file:///[WILDCARD]/browserslist/[WILDCARD]/index.js[WILDCARD]");
    assert_not_contains!(output.stderr(), "Updating V8 code cache");
  }
}

#[test]
fn code_cache_npm_cjs_wrapper_module_many_exports() {
  // The code cache was being invalidated because the CJS wrapper module
  // had indeterministic output.
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();
  temp_dir.write(
    "main.js",
    // this package has a few exports
    "import { hello } from \"npm:@denotest/cjs-reexport-collision\";hello.sayHello();",
  );

  // First run with no prior cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug -A main.js")
      .split_output()
      .run();

    assert_not_contains!(output.stderr(), "V8 code cache hit");
    assert_contains!(output.stderr(), "Updating V8 code cache");
    output.skip_stdout_check();
  }

  // 2nd run with cache.
  {
    let output = test_context
      .new_command()
      .args("run -Ldebug -A main.js")
      .split_output()
      .run();
    assert_contains!(output.stderr(), "V8 code cache hit");
    assert_not_contains!(output.stderr(), "Updating V8 code cache");
    output.skip_stdout_check();

    // should have two occurrences of this (one for entrypoint and one for wrapper module)
    assert_eq!(
      output
        .stderr()
        .split("V8 code cache hit for ES module")
        .count(),
      3
    );
  }
}

#[test]
fn node_process_stdin_unref_with_pty() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/node_process_stdin_unref_with_pty.js"])
    .with_pty(|mut console| {
      console.expect("START\r\n");
      console.write_line("foo");
      console.expect("foo\r\n");
      console.write_line("bar");
      console.expect("bar\r\n");
      console.write_line("baz");
      console.expect("baz\r\n");
    });

  TestContext::default()
    .new_command()
    .args_vec([
      "run",
      "--quiet",
      "run/node_process_stdin_unref_with_pty.js",
      "--unref",
    ])
    .with_pty(|mut console| {
      // if process.stdin.unref is called, the program immediately ends by skipping reading from stdin.
      console.expect("START\r\nEND\r\n");
    });
}

#[tokio::test]
async fn listen_tls_alpn() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--quiet")
    .arg("--allow-net")
    .arg("--allow-read")
    .arg("./cert/listen_tls_alpn.ts")
    .arg("4504")
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = child.stdout.as_mut().unwrap();
  let mut msg = [0; 5];
  let read = stdout.read(&mut msg).unwrap();
  assert_eq!(read, 5);
  assert_eq!(&msg, b"READY");

  let mut reader = &mut BufReader::new(Cursor::new(include_bytes!(
    "../testdata/tls/RootCA.crt"
  )));
  let certs = rustls_pemfile::certs(&mut reader)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  let mut root_store = rustls::RootCertStore::empty();
  root_store.add_parsable_certificates(certs);
  let mut cfg = rustls::ClientConfig::builder()
    .with_root_certificates(root_store)
    .with_no_client_auth();
  cfg.alpn_protocols.push(b"foobar".to_vec());
  let cfg = Arc::new(cfg);

  let hostname =
    rustls::pki_types::ServerName::try_from("localhost".to_string()).unwrap();

  let tcp_stream = tokio::net::TcpStream::connect("localhost:4504")
    .await
    .unwrap();
  let mut tls_stream = TlsStream::new_client_side(
    tcp_stream,
    ClientConnection::new(cfg, hostname).unwrap(),
    None,
  );

  let handshake = tls_stream.handshake().await.unwrap();

  assert_eq!(handshake.alpn, Some(b"foobar".to_vec()));

  let status = child.wait().unwrap();
  assert!(status.success());
}

#[tokio::test]
async fn listen_tls_alpn_fail() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--quiet")
    .arg("--allow-net")
    .arg("--allow-read")
    .arg("--config")
    .arg("../config/deno.json")
    .arg("./cert/listen_tls_alpn_fail.ts")
    .arg("4505")
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = child.stdout.as_mut().unwrap();
  let mut msg = [0; 5];
  let read = stdout.read(&mut msg).unwrap();
  assert_eq!(read, 5);
  assert_eq!(&msg, b"READY");

  let mut reader = &mut BufReader::new(Cursor::new(include_bytes!(
    "../testdata/tls/RootCA.crt"
  )));
  let certs = rustls_pemfile::certs(&mut reader)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  let mut root_store = rustls::RootCertStore::empty();
  root_store.add_parsable_certificates(certs);
  let mut cfg = rustls::ClientConfig::builder()
    .with_root_certificates(root_store)
    .with_no_client_auth();
  cfg.alpn_protocols.push(b"boofar".to_vec());
  let cfg = Arc::new(cfg);

  let hostname = rustls::pki_types::ServerName::try_from("localhost").unwrap();

  let tcp_stream = tokio::net::TcpStream::connect("localhost:4505")
    .await
    .unwrap();
  let mut tls_stream = TlsStream::new_client_side(
    tcp_stream,
    ClientConnection::new(cfg, hostname).unwrap(),
    None,
  );

  tls_stream.handshake().await.unwrap_err();

  let status = child.wait().unwrap();
  assert!(status.success());
}

// Couldn't get the directory readonly on windows on the CI
// so gave up because this being tested on unix is good enough
#[cfg(unix)]
#[test]
fn emit_failed_readonly_file_system() {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  context.deno_dir().path().canonicalize().make_dir_readonly();
  let temp_dir = context.temp_dir().path().canonicalize();
  temp_dir.join("main.ts").write("import './other.ts';");
  temp_dir.join("other.ts").write("console.log('hi');");
  let output = context
    .new_command()
    .args("run --log-level=debug main.ts")
    .run();
  output.assert_matches_text("[WILDCARD]Error saving emit data ([WILDLINE]main.ts)[WILDCARD]Skipped emit cache save of [WILDLINE]other.ts[WILDCARD]hi[WILDCARD]");
}

// todo(dsherret): waiting on fix in https://github.com/servo/rust-url/issues/505
#[ignore]
#[cfg(windows)]
#[test]
fn handle_invalid_path_error() {
  let deno_cmd = util::deno_cmd_with_deno_dir(&util::new_deno_dir());
  let output = deno_cmd.arg("run").arg("file://asdf").output().unwrap();
  assert_contains!(
    String::from_utf8_lossy(&output.stderr),
    "Invalid file path."
  );

  let deno_cmd = util::deno_cmd_with_deno_dir(&util::new_deno_dir());
  let output = deno_cmd.arg("run").arg("/a/b").output().unwrap();
  assert_contains!(String::from_utf8_lossy(&output.stderr), "Module not found");

  let deno_cmd = util::deno_cmd_with_deno_dir(&util::new_deno_dir());
  let output = deno_cmd.arg("run").arg("//a/b").output().unwrap();
  assert_contains!(
    String::from_utf8_lossy(&output.stderr),
    "Invalid file path."
  );

  let deno_cmd = util::deno_cmd_with_deno_dir(&util::new_deno_dir());
  let output = deno_cmd.arg("run").arg("///a/b").output().unwrap();
  assert_contains!(String::from_utf8_lossy(&output.stderr), "Module not found");
}
