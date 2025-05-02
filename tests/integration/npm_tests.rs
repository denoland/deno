// Copyright 2018-2025 the Deno authors. MIT license.

use pretty_assertions::assert_eq;
use serde_json::json;
use serde_json::Value;
use test_util as util;
use test_util::itest;
use url::Url;
use util::assert_contains;
use util::env_vars_for_npm_tests;
use util::http_server;
use util::TestContextBuilder;

// NOTE: See how to make test npm packages at ./testdata/npm/README.md

// FIXME(bartlomieju): npm: specifiers are not handled in dynamic imports
// at the moment
// itest!(dynamic_import {
//   args: "run --allow-read --allow-env npm/dynamic_import/main.ts",
//   output: "npm/dynamic_import/main.out",
//   envs: env_vars_for_npm_tests(),
//   http_server: true,
// });

itest!(run_existing_npm_package {
  args: "run --allow-read --node-modules-dir=auto npm:@denotest/bin",
  output: "npm/run_existing_npm_package/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  temp_cwd: true,
  cwd: Some("npm/run_existing_npm_package/"),
  copy_temp_dir: Some("npm/run_existing_npm_package/"),
});

itest!(require_resolve_url_paths {
  args: "run -A --quiet --node-modules-dir=auto url_paths.ts",
  output: "npm/require_resolve_url/url_paths.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  exit_code: 0,
  cwd: Some("npm/require_resolve_url/"),
  copy_temp_dir: Some("npm/require_resolve_url/"),
});

#[test]
fn parallel_downloading() {
  let (out, _err) = util::run_and_collect_output_with_args(
    true,
    vec![
      "run",
      "--allow-read",
      "--allow-env",
      "npm/cjs_with_deps/main.js",
    ],
    None,
    // don't use the sync env var
    Some(env_vars_for_npm_tests()),
    true,
  );
  assert!(out.contains("chalk cjs loads"));
}

#[test]
fn cached_only_after_first_run() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("npm/cached_only_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(stderr, "Download");
  assert_contains!(stdout, "[Function: chalk] createChalk");
  assert!(output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--cached-only")
    .arg("npm/cached_only_after_first_run/main2.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(
    stderr,
    "npm package not found in cache: \"ansi-styles\", --cached-only is specified."
  );
  assert!(stdout.is_empty());
  assert!(!output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--cached-only")
    .arg("npm/cached_only_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();

  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(output.status.success());
  assert!(stderr.is_empty());
  assert_contains!(stdout, "[Function: chalk] createChalk");
}

#[test]
fn reload_flag() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("npm/reload/main.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(stderr, "Download");
  assert_contains!(stdout, "[Function: chalk] createChalk");
  assert!(output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--reload")
    .arg("npm/reload/main.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(stderr, "Download");
  assert_contains!(stdout, "[Function: chalk] createChalk");
  assert!(output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--reload=npm:")
    .arg("npm/reload/main.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(stderr, "Download");
  assert_contains!(stdout, "[Function: chalk] createChalk");
  assert!(output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--reload=npm:chalk")
    .arg("npm/reload/main.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(stderr, "Download");
  assert_contains!(stdout, "[Function: chalk] createChalk");
  assert!(output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--reload=npm:foobar")
    .arg("npm/reload/main.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stderr.is_empty());
  assert_contains!(stdout, "[Function: chalk] createChalk");
  assert!(output.status.success());
}

#[test]
fn no_npm_after_first_run() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--no-npm")
    .arg("npm/no_npm_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(
    stderr,
    "error: npm specifiers were requested; but --no-npm is specified\n    at file:///"
  );
  assert!(stdout.is_empty());
  assert!(!output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("npm/no_npm_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(stderr, "Download");
  assert_contains!(stdout, "[Function: chalk] createChalk");
  assert!(output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--no-npm")
    .arg("npm/no_npm_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(
    stderr,
    "error: npm specifiers were requested; but --no-npm is specified\n    at file:///"
  );
  assert!(stdout.is_empty());
  assert!(!output.status.success());
}

#[test]
fn deno_run_cjs_module() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(deno_dir.path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--allow-write")
    .arg("npm:mkdirp@1.0.4")
    .arg("test_dir")
    .env("NO_COLOR", "1")
    .envs(env_vars_for_npm_tests())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert!(output.status.success());

  assert!(deno_dir.path().join("test_dir").exists());
}

#[test]
fn deno_run_bin_lockfile() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", "{}");
  let output = context
    .new_command()
    .args("run -A --quiet npm:@denotest/bin/cli-esm this is a test")
    .run();
  output.assert_matches_file("npm/deno_run_esm.out");
  assert!(temp_dir.path().join("deno.lock").exists());
}

#[test]
fn node_modules_dir_cache() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(deno_dir.path())
    .arg("cache")
    .arg("--node-modules-dir=auto")
    .arg("--quiet")
    .arg(util::testdata_path().join("npm/dual_cjs_esm/main.ts"))
    .envs(env_vars_for_npm_tests())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert!(output.status.success());

  let node_modules = deno_dir.path().join("node_modules");
  assert!(node_modules
    .join(
      ".deno/@denotest+dual-cjs-esm@1.0.0/node_modules/@denotest/dual-cjs-esm"
    )
    .exists());
  assert!(node_modules.join("@denotest/dual-cjs-esm").exists());

  // now try deleting the folder with the package source in the npm cache dir
  let package_global_cache_dir = deno_dir
    .path()
    .join("npm")
    .join("localhost_4260")
    .join("@denotest")
    .join("dual-cjs-esm")
    .join("1.0.0");
  assert!(package_global_cache_dir.exists());
  std::fs::remove_dir_all(&package_global_cache_dir).unwrap();

  // run the output, and it shouldn't bother recreating the directory
  // because it already has everything cached locally in the node_modules folder
  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(deno_dir.path())
    .arg("run")
    .arg("--node-modules-dir=auto")
    .arg("--quiet")
    .arg("-A")
    .arg(util::testdata_path().join("npm/dual_cjs_esm/main.ts"))
    .envs(env_vars_for_npm_tests())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert!(output.status.success());

  // this won't exist, but actually the parent directory
  // will because it still re-downloads the registry information
  assert!(!package_global_cache_dir.exists());
}

#[test]
fn ensure_registry_files_local() {
  // ensures the registry files all point at local tarballs
  let registry_dir_path = util::tests_path().join("registry").join("npm");
  for entry in walkdir::WalkDir::new(&registry_dir_path).max_depth(2) {
    let entry = entry.unwrap();
    if entry.metadata().unwrap().is_dir() {
      let registry_json_path = entry.path().join("registry.json");

      if registry_json_path.exists() {
        let file_text = std::fs::read_to_string(&registry_json_path).unwrap();
        if file_text.contains(&format!(
          "https://registry.npmjs.org/{}/-/",
          entry
            .path()
            .strip_prefix(&registry_dir_path)
            .unwrap()
            .to_string_lossy()
        )) {
          panic!(
            "file {} contained a reference to the npm registry",
            registry_json_path.display()
          );
        }
      }
    }
  }
}

#[test]
fn lock_file_missing_top_level_package() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();
  let temp_dir = util::TempDir::new();

  // write empty config file
  temp_dir.write("deno.json", "{}");

  // Lock file that is automatically picked up has been intentionally broken,
  // by removing "cowsay" package from it. This test ensures that npm resolver
  // snapshot can be successfully hydrated in such situation
  let lock_file_content = r#"{
    "version": "2",
    "remote": {},
    "npm": {
      "specifiers": { "cowsay": "cowsay@1.5.0" },
      "packages": {
        "ansi-regex@3.0.1": {
          "integrity": "sha512-+O9Jct8wf++lXxxFc4hc8LsjaSq0HFzzL7cVsw8pRDIPdjKD2mT4ytDZlLuSBZ4cLKZFXIrMGO7DbQCtMJJMKw==",
          "dependencies": {}
        },
        "ansi-regex@5.0.1": {
          "integrity": "sha512-quJQXlTSUGL2LH9SUXo8VwsY4soanhgo6LNSm84E1LBcE8s3O0wpdiRzyR9z/ZZJMlMWv37qOOb9pdJlMUEKFQ==",
          "dependencies": {}
        },
        "ansi-styles@4.3.0": {
          "integrity": "sha512-zbB9rCJAT1rbjiVDb2hqKFHNYLxgtk8NURxZ3IZwD3F6NtxbXZQCnnSi1Lkx+IDohdPlFp222wVALIheZJQSEg==",
          "dependencies": { "color-convert": "color-convert@2.0.1" }
        },
        "camelcase@5.3.1": {
          "integrity": "sha512-L28STB170nwWS63UjtlEOE3dldQApaJXZkOI1uMFfzf3rRuPegHaHesyee+YxQ+W6SvRDQV6UrdOdRiR153wJg==",
          "dependencies": {}
        },
        "cliui@6.0.0": {
          "integrity": "sha512-t6wbgtoCXvAzst7QgXxJYqPt0usEfbgQdftEPbLL/cvv6HPE5VgvqCuAIDR0NgU52ds6rFwqrgakNLrHEjCbrQ==",
          "dependencies": {
            "string-width": "string-width@4.2.3",
            "strip-ansi": "strip-ansi@6.0.1",
            "wrap-ansi": "wrap-ansi@6.2.0"
          }
        },
        "color-convert@2.0.1": {
          "integrity": "sha512-RRECPsj7iu/xb5oKYcsFHSppFNnsj/52OVTRKb4zP5onXwVF3zVmmToNcOfGC+CRDpfK/U584fMg38ZHCaElKQ==",
          "dependencies": { "color-name": "color-name@1.1.4" }
        },
        "color-name@1.1.4": {
          "integrity": "sha512-dOy+3AuW3a2wNbZHIuMZpTcgjGuLU/uBL/ubcZF9OXbDo8ff4O8yVp5Bf0efS8uEoYo5q4Fx7dY9OgQGXgAsQA==",
          "dependencies": {}
        },
        "decamelize@1.2.0": {
          "integrity": "sha512-z2S+W9X73hAUUki+N+9Za2lBlun89zigOyGrsax+KUQ6wKW4ZoWpEYBkGhQjwAjjDCkWxhY0VKEhk8wzY7F5cA==",
          "dependencies": {}
        },
        "emoji-regex@8.0.0": {
          "integrity": "sha512-MSjYzcWNOA0ewAHpz0MxpYFvwg6yjy1NG3xteoqz644VCo/RPgnr1/GGt+ic3iJTzQ8Eu3TdM14SawnVUmGE6A==",
          "dependencies": {}
        },
        "find-up@4.1.0": {
          "integrity": "sha512-PpOwAdQ/YlXQ2vj8a3h8IipDuYRi3wceVQQGYWxNINccq40Anw7BlsEXCMbt1Zt+OLA6Fq9suIpIWD0OsnISlw==",
          "dependencies": {
            "locate-path": "locate-path@5.0.0",
            "path-exists": "path-exists@4.0.0"
          }
        },
        "get-caller-file@2.0.5": {
          "integrity": "sha512-DyFP3BM/3YHTQOCUL/w0OZHR0lpKeGrxotcHWcqNEdnltqFwXVfhEBQ94eIo34AfQpo0rGki4cyIiftY06h2Fg==",
          "dependencies": {}
        },
        "get-stdin@8.0.0": {
          "integrity": "sha512-sY22aA6xchAzprjyqmSEQv4UbAAzRN0L2dQB0NlN5acTTK9Don6nhoc3eAbUnpZiCANAMfd/+40kVdKfFygohg==",
          "dependencies": {}
        },
        "is-fullwidth-code-point@2.0.0": {
          "integrity": "sha512-VHskAKYM8RfSFXwee5t5cbN5PZeq1Wrh6qd5bkyiXIf6UQcN6w/A0eXM9r6t8d+GYOh+o6ZhiEnb88LN/Y8m2w==",
          "dependencies": {}
        },
        "is-fullwidth-code-point@3.0.0": {
          "integrity": "sha512-zymm5+u+sCsSWyD9qNaejV3DFvhCKclKdizYaJUuHA83RLjb7nSuGnddCHGv0hk+KY7BMAlsWeK4Ueg6EV6XQg==",
          "dependencies": {}
        },
        "locate-path@5.0.0": {
          "integrity": "sha512-t7hw9pI+WvuwNJXwk5zVHpyhIqzg2qTlklJOf0mVxGSbe3Fp2VieZcduNYjaLDoy6p9uGpQEGWG87WpMKlNq8g==",
          "dependencies": { "p-locate": "p-locate@4.1.0" }
        },
        "p-limit@2.3.0": {
          "integrity": "sha512-//88mFWSJx8lxCzwdAABTJL2MyWB12+eIY7MDL2SqLmAkeKU9qxRvWuSyTjm3FUmpBEMuFfckAIqEaVGUDxb6w==",
          "dependencies": { "p-try": "p-try@2.2.0" }
        },
        "p-locate@4.1.0": {
          "integrity": "sha512-R79ZZ/0wAxKGu3oYMlz8jy/kbhsNrS7SKZ7PxEHBgJ5+F2mtFW2fK2cOtBh1cHYkQsbzFV7I+EoRKe6Yt0oK7A==",
          "dependencies": { "p-limit": "p-limit@2.3.0" }
        },
        "p-try@2.2.0": {
          "integrity": "sha512-R4nPAVTAU0B9D35/Gk3uJf/7XYbQcyohSKdvAxIRSNghFl4e71hVoGnBNQz9cWaXxO2I10KTC+3jMdvvoKw6dQ==",
          "dependencies": {}
        },
        "path-exists@4.0.0": {
          "integrity": "sha512-ak9Qy5Q7jYb2Wwcey5Fpvg2KoAc/ZIhLSLOSBmRmygPsGwkVVt0fZa0qrtMz+m6tJTAHfZQ8FnmB4MG4LWy7/w==",
          "dependencies": {}
        },
        "require-directory@2.1.1": {
          "integrity": "sha512-fGxEI7+wsG9xrvdjsrlmL22OMTTiHRwAMroiEeMgq8gzoLC/PQr7RsRDSTLUg/bZAZtF+TVIkHc6/4RIKrui+Q==",
          "dependencies": {}
        },
        "require-main-filename@2.0.0": {
          "integrity": "sha512-NKN5kMDylKuldxYLSUfrbo5Tuzh4hd+2E8NPPX02mZtn1VuREQToYe/ZdlJy+J3uCpfaiGF05e7B8W0iXbQHmg==",
          "dependencies": {}
        },
        "set-blocking@2.0.0": {
          "integrity": "sha512-KiKBS8AnWGEyLzofFfmvKwpdPzqiy16LvQfK3yv/fVH7Bj13/wl3JSR1J+rfgRE9q7xUJK4qvgS8raSOeLUehw==",
          "dependencies": {}
        },
        "string-width@2.1.1": {
          "integrity": "sha512-nOqH59deCq9SRHlxq1Aw85Jnt4w6KvLKqWVik6oA9ZklXLNIOlqg4F2yrT1MVaTjAqvVwdfeZ7w7aCvJD7ugkw==",
          "dependencies": {
            "is-fullwidth-code-point": "is-fullwidth-code-point@2.0.0",
            "strip-ansi": "strip-ansi@4.0.0"
          }
        },
        "string-width@4.2.3": {
          "integrity": "sha512-wKyQRQpjJ0sIp62ErSZdGsjMJWsap5oRNihHhu6G7JVO/9jIB6UyevL+tXuOqrng8j/cxKTWyWUwvSTriiZz/g==",
          "dependencies": {
            "emoji-regex": "emoji-regex@8.0.0",
            "is-fullwidth-code-point": "is-fullwidth-code-point@3.0.0",
            "strip-ansi": "strip-ansi@6.0.1"
          }
        },
        "strip-ansi@4.0.0": {
          "integrity": "sha512-4XaJ2zQdCzROZDivEVIDPkcQn8LMFSa8kj8Gxb/Lnwzv9A8VctNZ+lfivC/sV3ivW8ElJTERXZoPBRrZKkNKow==",
          "dependencies": { "ansi-regex": "ansi-regex@3.0.1" }
        },
        "strip-ansi@6.0.1": {
          "integrity": "sha512-Y38VPSHcqkFrCpFnQ9vuSXmquuv5oXOKpGeT6aGrr3o3Gc9AlVa6JBfUSOCnbxGGZF+/0ooI7KrPuUSztUdU5A==",
          "dependencies": { "ansi-regex": "ansi-regex@5.0.1" }
        },
        "strip-final-newline@2.0.0": {
          "integrity": "sha512-BrpvfNAE3dcvq7ll3xVumzjKjZQ5tI1sEUIKr3Uoks0XUl45St3FlatVqef9prk4jRDzhW6WZg+3bk93y6pLjA==",
          "dependencies": {}
        },
        "which-module@2.0.0": {
          "integrity": "sha512-B+enWhmw6cjfVC7kS8Pj9pCrKSc5txArRyaYGe088shv/FGWH+0Rjx/xPgtsWfsUtS27FkP697E4DDhgrgoc0Q==",
          "dependencies": {}
        },
        "wrap-ansi@6.2.0": {
          "integrity": "sha512-r6lPcBGxZXlIcymEu7InxDMhdW0KDxpLgoFLcguasxCaJ/SOIZwINatK9KY/tf+ZrlywOKU0UDj3ATXUBfxJXA==",
          "dependencies": {
            "ansi-styles": "ansi-styles@4.3.0",
            "string-width": "string-width@4.2.3",
            "strip-ansi": "strip-ansi@6.0.1"
          }
        },
        "y18n@4.0.3": {
          "integrity": "sha512-JKhqTOwSrqNA1NY5lSztJ1GrBiUodLMmIZuLiDaMRJ+itFd+ABVE8XBjOvIWL+rSqNDC74LCSFmlb/U4UZ4hJQ==",
          "dependencies": {}
        },
        "yargs-parser@18.1.3": {
          "integrity": "sha512-o50j0JeToy/4K6OZcaQmW6lyXXKhq7csREXcDwk2omFPJEwUNOVtJKvmDr9EI1fAJZUyZcRF7kxGBWmRXudrCQ==",
          "dependencies": {
            "camelcase": "camelcase@5.3.1",
            "decamelize": "decamelize@1.2.0"
          }
        },
        "yargs@15.4.1": {
          "integrity": "sha512-aePbxDmcYW++PaqBsJ+HYUFwCdv4LVvdnhBy78E57PIor8/OVvhMrADFFEDh8DHDFRv/O9i3lPhsENjO7QX0+A==",
          "dependencies": {
            "cliui": "cliui@6.0.0",
            "decamelize": "decamelize@1.2.0",
            "find-up": "find-up@4.1.0",
            "get-caller-file": "get-caller-file@2.0.5",
            "require-directory": "require-directory@2.1.1",
            "require-main-filename": "require-main-filename@2.0.0",
            "set-blocking": "set-blocking@2.0.0",
            "string-width": "string-width@4.2.3",
            "which-module": "which-module@2.0.0",
            "y18n": "y18n@4.0.3",
            "yargs-parser": "yargs-parser@18.1.3"
          }
        }
      }
    }
  }
  "#;
  temp_dir.write("deno.lock", lock_file_content);
  let main_contents = r#"
  import cowsay from "npm:cowsay";
  console.log(cowsay);
  "#;
  temp_dir.write("main.ts", main_contents);

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(temp_dir.path())
    .arg("run")
    .arg("--quiet")
    .arg("--lock")
    .arg("deno.lock")
    .arg("-A")
    .arg("main.ts")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert!(!output.status.success());

  let stderr = String::from_utf8(output.stderr).unwrap();
  assert_eq!(
    stderr,
    concat!(
      "error: failed reading lockfile 'deno.lock'\n",
      "\n",
      "Caused by:\n",
      "    0: The lockfile is corrupt. Remove the lockfile to regenerate it.\n",
      "    1: Could not find 'cowsay@1.5.0' in the list of packages.\n"
    )
  );
}

#[test]
fn lock_file_lock_write() {
  // https://github.com/denoland/deno/issues/16666
  let _server = http_server();

  let deno_dir = util::new_deno_dir();
  let temp_dir = util::TempDir::new();

  temp_dir.write("deno.json", "{}");
  let lock_file_content = r#"{
  "version": "5",
  "specifiers": {
    "npm:cowsay@1.5.0": "1.5.0"
  },
  "npm": {
    "ansi-regex@3.0.1": {
      "integrity": "sha512-+O9Jct8wf++lXxxFc4hc8LsjaSq0HFzzL7cVsw8pRDIPdjKD2mT4ytDZlLuSBZ4cLKZFXIrMGO7DbQCtMJJMKw=="
    },
    "ansi-regex@5.0.1": {
      "integrity": "sha512-quJQXlTSUGL2LH9SUXo8VwsY4soanhgo6LNSm84E1LBcE8s3O0wpdiRzyR9z/ZZJMlMWv37qOOb9pdJlMUEKFQ=="
    },
    "ansi-styles@4.3.0": {
      "integrity": "sha512-zbB9rCJAT1rbjiVDb2hqKFHNYLxgtk8NURxZ3IZwD3F6NtxbXZQCnnSi1Lkx+IDohdPlFp222wVALIheZJQSEg==",
      "dependencies": [
        "color-convert"
      ]
    },
    "camelcase@5.3.1": {
      "integrity": "sha512-L28STB170nwWS63UjtlEOE3dldQApaJXZkOI1uMFfzf3rRuPegHaHesyee+YxQ+W6SvRDQV6UrdOdRiR153wJg=="
    },
    "cliui@6.0.0": {
      "integrity": "sha512-t6wbgtoCXvAzst7QgXxJYqPt0usEfbgQdftEPbLL/cvv6HPE5VgvqCuAIDR0NgU52ds6rFwqrgakNLrHEjCbrQ==",
      "dependencies": [
        "string-width@4.2.3",
        "strip-ansi@6.0.1",
        "wrap-ansi"
      ]
    },
    "color-convert@2.0.1": {
      "integrity": "sha512-RRECPsj7iu/xb5oKYcsFHSppFNnsj/52OVTRKb4zP5onXwVF3zVmmToNcOfGC+CRDpfK/U584fMg38ZHCaElKQ==",
      "dependencies": [
        "color-name"
      ]
    },
    "color-name@1.1.4": {
      "integrity": "sha512-dOy+3AuW3a2wNbZHIuMZpTcgjGuLU/uBL/ubcZF9OXbDo8ff4O8yVp5Bf0efS8uEoYo5q4Fx7dY9OgQGXgAsQA=="
    },
    "cowsay@1.5.0": {
      "integrity": "sha512-8Ipzr54Z8zROr/62C8f0PdhQcDusS05gKTS87xxdji8VbWefWly0k8BwGK7+VqamOrkv3eGsCkPtvlHzrhWsCA==",
      "dependencies": [
        "get-stdin",
        "string-width@2.1.1",
        "strip-final-newline",
        "yargs"
      ]
    },
    "decamelize@1.2.0": {
      "integrity": "sha512-z2S+W9X73hAUUki+N+9Za2lBlun89zigOyGrsax+KUQ6wKW4ZoWpEYBkGhQjwAjjDCkWxhY0VKEhk8wzY7F5cA=="
    },
    "emoji-regex@8.0.0": {
      "integrity": "sha512-MSjYzcWNOA0ewAHpz0MxpYFvwg6yjy1NG3xteoqz644VCo/RPgnr1/GGt+ic3iJTzQ8Eu3TdM14SawnVUmGE6A=="
    },
    "find-up@4.1.0": {
      "integrity": "sha512-PpOwAdQ/YlXQ2vj8a3h8IipDuYRi3wceVQQGYWxNINccq40Anw7BlsEXCMbt1Zt+OLA6Fq9suIpIWD0OsnISlw==",
      "dependencies": [
        "locate-path",
        "path-exists"
      ]
    },
    "get-caller-file@2.0.5": {
      "integrity": "sha512-DyFP3BM/3YHTQOCUL/w0OZHR0lpKeGrxotcHWcqNEdnltqFwXVfhEBQ94eIo34AfQpo0rGki4cyIiftY06h2Fg=="
    },
    "get-stdin@8.0.0": {
      "integrity": "sha512-sY22aA6xchAzprjyqmSEQv4UbAAzRN0L2dQB0NlN5acTTK9Don6nhoc3eAbUnpZiCANAMfd/+40kVdKfFygohg=="
    },
    "is-fullwidth-code-point@2.0.0": {
      "integrity": "sha512-VHskAKYM8RfSFXwee5t5cbN5PZeq1Wrh6qd5bkyiXIf6UQcN6w/A0eXM9r6t8d+GYOh+o6ZhiEnb88LN/Y8m2w=="
    },
    "is-fullwidth-code-point@3.0.0": {
      "integrity": "sha512-zymm5+u+sCsSWyD9qNaejV3DFvhCKclKdizYaJUuHA83RLjb7nSuGnddCHGv0hk+KY7BMAlsWeK4Ueg6EV6XQg=="
    },
    "locate-path@5.0.0": {
      "integrity": "sha512-t7hw9pI+WvuwNJXwk5zVHpyhIqzg2qTlklJOf0mVxGSbe3Fp2VieZcduNYjaLDoy6p9uGpQEGWG87WpMKlNq8g==",
      "dependencies": [
        "p-locate"
      ]
    },
    "p-limit@2.3.0": {
      "integrity": "sha512-//88mFWSJx8lxCzwdAABTJL2MyWB12+eIY7MDL2SqLmAkeKU9qxRvWuSyTjm3FUmpBEMuFfckAIqEaVGUDxb6w==",
      "dependencies": [
        "p-try"
      ]
    },
    "p-locate@4.1.0": {
      "integrity": "sha512-R79ZZ/0wAxKGu3oYMlz8jy/kbhsNrS7SKZ7PxEHBgJ5+F2mtFW2fK2cOtBh1cHYkQsbzFV7I+EoRKe6Yt0oK7A==",
      "dependencies": [
        "p-limit"
      ]
    },
    "p-try@2.2.0": {
      "integrity": "sha512-R4nPAVTAU0B9D35/Gk3uJf/7XYbQcyohSKdvAxIRSNghFl4e71hVoGnBNQz9cWaXxO2I10KTC+3jMdvvoKw6dQ=="
    },
    "path-exists@4.0.0": {
      "integrity": "sha512-ak9Qy5Q7jYb2Wwcey5Fpvg2KoAc/ZIhLSLOSBmRmygPsGwkVVt0fZa0qrtMz+m6tJTAHfZQ8FnmB4MG4LWy7/w=="
    },
    "require-directory@2.1.1": {
      "integrity": "sha512-fGxEI7+wsG9xrvdjsrlmL22OMTTiHRwAMroiEeMgq8gzoLC/PQr7RsRDSTLUg/bZAZtF+TVIkHc6/4RIKrui+Q=="
    },
    "require-main-filename@2.0.0": {
      "integrity": "sha512-NKN5kMDylKuldxYLSUfrbo5Tuzh4hd+2E8NPPX02mZtn1VuREQToYe/ZdlJy+J3uCpfaiGF05e7B8W0iXbQHmg=="
    },
    "set-blocking@2.0.0": {
      "integrity": "sha512-KiKBS8AnWGEyLzofFfmvKwpdPzqiy16LvQfK3yv/fVH7Bj13/wl3JSR1J+rfgRE9q7xUJK4qvgS8raSOeLUehw=="
    },
    "string-width@2.1.1": {
      "integrity": "sha512-nOqH59deCq9SRHlxq1Aw85Jnt4w6KvLKqWVik6oA9ZklXLNIOlqg4F2yrT1MVaTjAqvVwdfeZ7w7aCvJD7ugkw==",
      "dependencies": [
        "is-fullwidth-code-point@2.0.0",
        "strip-ansi@4.0.0"
      ]
    },
    "string-width@4.2.3": {
      "integrity": "sha512-wKyQRQpjJ0sIp62ErSZdGsjMJWsap5oRNihHhu6G7JVO/9jIB6UyevL+tXuOqrng8j/cxKTWyWUwvSTriiZz/g==",
      "dependencies": [
        "emoji-regex",
        "is-fullwidth-code-point@3.0.0",
        "strip-ansi@6.0.1"
      ]
    },
    "strip-ansi@4.0.0": {
      "integrity": "sha512-4XaJ2zQdCzROZDivEVIDPkcQn8LMFSa8kj8Gxb/Lnwzv9A8VctNZ+lfivC/sV3ivW8ElJTERXZoPBRrZKkNKow==",
      "dependencies": [
        "ansi-regex@3.0.1"
      ]
    },
    "strip-ansi@6.0.1": {
      "integrity": "sha512-Y38VPSHcqkFrCpFnQ9vuSXmquuv5oXOKpGeT6aGrr3o3Gc9AlVa6JBfUSOCnbxGGZF+/0ooI7KrPuUSztUdU5A==",
      "dependencies": [
        "ansi-regex@5.0.1"
      ]
    },
    "strip-final-newline@2.0.0": {
      "integrity": "sha512-BrpvfNAE3dcvq7ll3xVumzjKjZQ5tI1sEUIKr3Uoks0XUl45St3FlatVqef9prk4jRDzhW6WZg+3bk93y6pLjA=="
    },
    "which-module@2.0.0": {
      "integrity": "sha512-B+enWhmw6cjfVC7kS8Pj9pCrKSc5txArRyaYGe088shv/FGWH+0Rjx/xPgtsWfsUtS27FkP697E4DDhgrgoc0Q=="
    },
    "wrap-ansi@6.2.0": {
      "integrity": "sha512-r6lPcBGxZXlIcymEu7InxDMhdW0KDxpLgoFLcguasxCaJ/SOIZwINatK9KY/tf+ZrlywOKU0UDj3ATXUBfxJXA==",
      "dependencies": [
        "ansi-styles",
        "string-width@4.2.3",
        "strip-ansi@6.0.1"
      ]
    },
    "y18n@4.0.3": {
      "integrity": "sha512-JKhqTOwSrqNA1NY5lSztJ1GrBiUodLMmIZuLiDaMRJ+itFd+ABVE8XBjOvIWL+rSqNDC74LCSFmlb/U4UZ4hJQ=="
    },
    "yargs-parser@18.1.3": {
      "integrity": "sha512-o50j0JeToy/4K6OZcaQmW6lyXXKhq7csREXcDwk2omFPJEwUNOVtJKvmDr9EI1fAJZUyZcRF7kxGBWmRXudrCQ==",
      "dependencies": [
        "camelcase",
        "decamelize"
      ]
    },
    "yargs@15.4.1": {
      "integrity": "sha512-aePbxDmcYW++PaqBsJ+HYUFwCdv4LVvdnhBy78E57PIor8/OVvhMrADFFEDh8DHDFRv/O9i3lPhsENjO7QX0+A==",
      "dependencies": [
        "cliui",
        "decamelize",
        "find-up",
        "get-caller-file",
        "require-directory",
        "require-main-filename",
        "set-blocking",
        "string-width@4.2.3",
        "which-module",
        "y18n",
        "yargs-parser"
      ]
    }
  }
}
"#;
  temp_dir.write("deno.lock", lock_file_content);

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(temp_dir.path())
    .arg("cache")
    .arg("--quiet")
    .arg("npm:cowsay@1.5.0")
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  eprintln!(
    "output: {}",
    String::from_utf8(output.stderr.clone()).unwrap()
  );
  assert!(output.status.success());
  assert_eq!(output.status.code(), Some(0));

  let stdout = String::from_utf8(output.stdout).unwrap();
  assert!(stdout.is_empty());
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert!(stderr.is_empty());
  assert_eq!(
    std::fs::read_to_string(temp_dir.path().join("deno.lock")).unwrap(),
    lock_file_content,
  );
}

#[test]
fn auto_discover_lock_file() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();

  let temp_dir = context.temp_dir();

  // write empty config file
  temp_dir.write("deno.json", "{}");

  // write a lock file with borked integrity
  let lock_file_content = r#"{
    "version": "5",
    "specifiers": {
      "npm:@denotest/bin": "1.0.0"
    },
    "npm": {
      "@denotest/bin@1.0.0": {
        "integrity": "sha512-foobar",
        "tarball": "http://localhost:4260/@denotest/bin/1.0.0.tgz"
      }
    }
  }"#;
  temp_dir.write("deno.lock", lock_file_content);

  let output = context
    .new_command()
    .args("run -A npm:@denotest/bin/cli-esm test")
    .run();
  output
    .assert_matches_text(
r#"Download http://localhost:4260/@denotest/bin/1.0.0.tgz
error: Failed caching npm package '@denotest/bin@1.0.0'

Caused by:
    Tarball checksum did not match what was provided by npm registry for @denotest/bin@1.0.0.
    
    Expected: foobar
    Actual: [WILDCARD]
"#)
    .assert_exit_code(1);
}

// TODO(2.0): this should be rewritten to a spec test and first run `deno install`
// itest!(node_modules_import_run {
//   args: "run --quiet main.ts",
//   output: "npm/node_modules_import/main.out",
//   http_server: true,
//   copy_temp_dir: Some("npm/node_modules_import/"),
//   cwd: Some("npm/node_modules_import/"),
//   envs: env_vars_for_npm_tests(),
//   exit_code: 0,
// });

// TODO(2.0): this should be rewritten to a spec test and first run `deno install`
// itest!(node_modules_import_check {
//   args: "check --quiet main.ts",
//   output: "npm/node_modules_import/main_check.out",
//   envs: env_vars_for_npm_tests(),
//   http_server: true,
//   cwd: Some("npm/node_modules_import/"),
//   copy_temp_dir: Some("npm/node_modules_import/"),
//   exit_code: 1,
// });

// TODO(2.0): this should be rewritten to a spec test and first run `deno install`
#[test]
#[ignore]
fn reload_info_not_found_cache_but_exists_remote() {
  fn remove_version(registry_json: &mut Value, version: &str) {
    registry_json
      .as_object_mut()
      .unwrap()
      .get_mut("versions")
      .unwrap()
      .as_object_mut()
      .unwrap()
      .remove(version);
  }

  fn remove_version_for_package(
    deno_dir: &util::TempDir,
    package: &str,
    version: &str,
  ) {
    let registry_json_path =
      format!("npm/localhost_4260/{}/registry.json", package);
    let mut registry_json: Value =
      serde_json::from_str(&deno_dir.read_to_string(&registry_json_path))
        .unwrap();
    remove_version(&mut registry_json, version);
    // for the purpose of this test, just remove the dist-tag as it might contain this version
    registry_json
      .as_object_mut()
      .unwrap()
      .get_mut("dist-tags")
      .unwrap()
      .as_object_mut()
      .unwrap()
      .remove("latest");
    deno_dir.write(
      &registry_json_path,
      serde_json::to_string(&registry_json).unwrap(),
    );
  }

  // This tests that when a local machine doesn't have a version
  // specified in a dependency that exists in the npm registry
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let deno_dir = test_context.deno_dir();
  let temp_dir = test_context.temp_dir();
  temp_dir.write(
    "main.ts",
    "import 'npm:@denotest/esm-import-cjs-default@1.0.0';",
  );

  // cache successfully to the deno_dir
  let output = test_context
    .new_command()
    .args("cache main.ts npm:@denotest/esm-basic@1.0.0")
    .run();
  output.assert_matches_text(concat!(
    "[UNORDERED_START]\n",
    "Download http://localhost:4260/@denotest%2fesm-basic\n",
    "Download http://localhost:4260/@denotest%2fesm-import-cjs-default\n",
    "Download http://localhost:4260/@denotest%2fcjs-default-export\n",
    "Download http://localhost:4260/@denotestcjs-default-export/1.0.0.tgz\n",
    "Download http://localhost:4260/@denotest/esm-basic/1.0.0.tgz\n",
    "Download http://localhost:4260/@denotest/esm-import-cjs-default/1.0.0.tgz\n",
    "[UNORDERED_END]\n",
  ));

  // test in dependency
  {
    // modify the package information in the cache to remove the latest version
    remove_version_for_package(
      deno_dir,
      "@denotest/cjs-default-export",
      "1.0.0",
    );

    // should error when `--cache-only` is used now because the version is not in the cache
    let output = test_context
      .new_command()
      .args("run --cached-only main.ts")
      .run();
    output.assert_exit_code(1);
    output.assert_matches_text("error: Could not find npm package '@denotest/cjs-default-export' matching '^1.0.0'.\n");

    // now try running without it, it should download the package now
    let output = test_context.new_command().args("run main.ts").run();
    output.assert_matches_text(concat!(
      "[UNORDERED_START]\n",
      "Download http://localhost:4260/@denotest%2fesm-import-cjs-default\n",
      "Download http://localhost:4260/@denotest%2fcjs-default-export\n",
      "[UNORDERED_END]\n",
      "Node esm importing node cjs\n[WILDCARD]",
    ));
    output.assert_exit_code(0);
  }

  // test in npm specifier
  {
    // now remove the information for the top level package
    remove_version_for_package(
      deno_dir,
      "@denotest/esm-import-cjs-default",
      "1.0.0",
    );

    // should error for --cached-only
    let output = test_context
      .new_command()
      .args("run --cached-only main.ts")
      .run();
    output.assert_matches_text(concat!(
      "error: Could not find npm package '@denotest/esm-import-cjs-default' matching '1.0.0'.\n",
      "    at file:///[WILDCARD]/main.ts:1:8\n",
    ));
    output.assert_exit_code(1);

    // now try running, it should work
    let output = test_context.new_command().args("run main.ts").run();
    output.assert_matches_text(concat!(
      "[UNORDERED_START]\n",
      "Download http://localhost:4260/@denotest%2fesm-import-cjs-default\n",
      "Download http://localhost:4260/@denotest%2fcjs-default-export\n",
      "[UNORDERED_END]\n",
      "Node esm importing node cjs\n[WILDCARD]",
    ));
    output.assert_exit_code(0);
  }

  // test matched specifier in package.json
  {
    // write out a package.json and a new main.ts with a bare specifier
    temp_dir.write("main.ts", "import '@denotest/esm-import-cjs-default';");
    temp_dir.write(
      "package.json",
      r#"{ "dependencies": { "@denotest/esm-import-cjs-default": "1.0.0" }}"#,
    );

    // remove the top level package information again
    remove_version_for_package(
      deno_dir,
      "@denotest/esm-import-cjs-default",
      "1.0.0",
    );

    // should error for --cached-only
    let output = test_context
      .new_command()
      .args("run --cached-only main.ts")
      .run();
    output.assert_matches_text(concat!(
      "error: Could not find npm package '@denotest/esm-import-cjs-default' matching '1.0.0'.\n",
      "    at file:///[WILDCARD]/main.ts:1:8\n",
    ));
    output.assert_exit_code(1);

    // now try running, it should work
    let output = test_context.new_command().args("run main.ts").run();
    output.assert_matches_text(concat!(
      "[UNORDERED_START]\n",
      "Download http://localhost:4260/@denotest%2fesm-import-cjs-default\n",
      "Download http://localhost:4260/@denotest%2fcjs-default-export\n",
      "[UNORDERED_END]\n",
      "[UNORDERED_START]\n",
      "Initialize @denotest/cjs-default-export@1.0.0\n",
      "Initialize @denotest/esm-import-cjs-default@1.0.0\n",
      "[UNORDERED_END]\n",
      "Node esm importing node cjs\n[WILDCARD]",
    ));
    output.assert_exit_code(0);
  }

  // temp other dependency in package.json
  {
    // write out a package.json that has another dependency
    temp_dir.write(
      "package.json",
      r#"{ "dependencies": { "@denotest/esm-import-cjs-default": "1.0.0", "@denotest/esm-basic": "1.0.0" }}"#,
    );

    // remove the dependency's version
    remove_version_for_package(deno_dir, "@denotest/esm-basic", "1.0.0");

    // should error for --cached-only
    let output = test_context
      .new_command()
      .args("run --cached-only main.ts")
      .run();
    output.assert_matches_text(concat!(
      "error: Could not find npm package '@denotest/esm-basic' matching '1.0.0'.\n",
    ));
    output.assert_exit_code(1);

    // now try running, it should work and only initialize the new package
    let output = test_context.new_command().args("run main.ts").run();
    output.assert_matches_text(concat!(
      "[UNORDERED_START]\n",
      "Download http://localhost:4260/@denotest%2fesm-basic\n",
      "Download http://localhost:4260/@denotest%2fesm-import-cjs-default\n",
      "Download http://localhost:4260/@denotest%2fcjs-default-export\n",
      "[UNORDERED_END]\n",
      "Initialize @denotest/esm-basic@1.0.0\n",
      "Node esm importing node cjs\n[WILDCARD]",
    ));
    output.assert_exit_code(0);
  }

  // now try using a lockfile
  {
    // create it
    temp_dir.write("deno.json", r#"{}"#);
    test_context
      .new_command()
      .args("install --entrypoint main.ts")
      .run();
    assert!(temp_dir.path().join("deno.lock").exists());

    // remove a version found in the lockfile
    remove_version_for_package(deno_dir, "@denotest/esm-basic", "1.0.0");

    // should error for --cached-only
    let output = test_context
      .new_command()
      .args("run --cached-only main.ts")
      .run();
    output.assert_matches_text(concat!(
      "error: failed reading lockfile '[WILDCARD]deno.lock'\n",
      "\n",
      "Caused by:\n",
      "    0: Could not find '@denotest/esm-basic@1.0.0' specified in the lockfile.\n",
      "    1: Could not find version '1.0.0' for npm package '@denotest/esm-basic'.\n",
    ));
    output.assert_exit_code(1);

    // now try running, it should work and only initialize the new package
    let output = test_context.new_command().args("run main.ts").run();
    output.assert_matches_text(concat!(
      "[UNORDERED_START]\n",
      "Download http://localhost:4260/@denotest%2fcjs-default-export\n",
      "Download http://localhost:4260/@denotest%2fesm-basic\n",
      "Download http://localhost:4260/@denotest%2fesm-import-cjs-default\n",
      "[UNORDERED_END]\n",
      "Node esm importing node cjs\n[WILDCARD]",
    ));
    output.assert_exit_code(0);
  }
}

#[test]
fn binary_package_with_optional_dependencies() {
  let context = TestContextBuilder::for_npm()
    .use_copy_temp_dir("npm/binary_package")
    .cwd("npm/binary_package")
    .build();

  let temp_dir = context.temp_dir();
  let temp_dir_path = temp_dir.path();
  let project_path = temp_dir_path.join("npm/binary_package");

  // write empty config file so a lockfile gets created
  temp_dir.write("npm/binary_package/deno.json", "{}");

  // run it twice, with the first time creating the lockfile and the second using it
  for i in 0..2 {
    if i == 1 {
      assert!(project_path.join("deno.lock").exists());
    }

    let output = context
      .new_command()
      .args("run -A --node-modules-dir=auto main.js")
      .run();

    #[cfg(target_os = "windows")]
    {
      output.assert_exit_code(0);
      output.assert_matches_text(
        "[WILDCARD]Hello from binary package on windows[WILDCARD]",
      );
      assert!(project_path
        .join("node_modules/.deno/@denotest+binary-package-windows@1.0.0")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package-linux@1.0.0")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package-mac@1.0.0")
        .exists());
      assert!(project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-windows")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-linux")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-mac")
        .exists());
    }

    #[cfg(target_os = "macos")]
    {
      output.assert_exit_code(0);
      output.assert_matches_text(
        "[WILDCARD]Hello from binary package on mac[WILDCARD]",
      );

      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package-windows@1.0.0")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package-linux@1.0.0")
        .exists());
      assert!(project_path
        .join("node_modules/.deno/@denotest+binary-package-mac@1.0.0")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-windows")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-linux")
        .exists());
      assert!(project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-mac")
        .exists());
    }

    #[cfg(target_os = "linux")]
    {
      output.assert_exit_code(0);
      output.assert_matches_text(
        "[WILDCARD]Hello from binary package on linux[WILDCARD]",
      );
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package-windows@1.0.0")
        .exists());
      assert!(project_path
        .join("node_modules/.deno/@denotest+binary-package-linux@1.0.0")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package-mac@1.0.0")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-windows")
        .exists());
      assert!(project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-linux")
        .exists());
      assert!(!project_path
        .join("node_modules/.deno/@denotest+binary-package@1.0.0/node_modules/@denotest/binary-package-mac")
        .exists());
    }
  }
}

#[test]
fn node_modules_dir_config_file() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();
  let node_modules_dir = temp_dir.path().join("node_modules");
  let rm_node_modules = || std::fs::remove_dir_all(&node_modules_dir).unwrap();

  temp_dir.write("deno.json", r#"{ "nodeModulesDir": "auto" }"#);
  temp_dir.write("main.ts", "import 'npm:@denotest/esm-basic';");

  let deno_cache_cmd = test_context.new_command().args("cache --quiet main.ts");
  deno_cache_cmd.run();
  assert!(node_modules_dir.exists());

  // now try adding a vendor flag, it should exist
  rm_node_modules();
  temp_dir.write("deno.json", r#"{ "vendor": true }"#);
  deno_cache_cmd.run();
  assert!(node_modules_dir.exists());

  rm_node_modules();
  temp_dir.write("deno.json", r#"{ "nodeModulesDir": "none" }"#);

  deno_cache_cmd.run();
  assert!(!node_modules_dir.exists());

  temp_dir.write("package.json", r#"{}"#);
  deno_cache_cmd.run();
  assert!(!node_modules_dir.exists());

  test_context
    .new_command()
    .args("cache --quiet --node-modules-dir=auto main.ts")
    .run();
  assert!(node_modules_dir.exists());

  // should override the `--vendor` flag
  rm_node_modules();
  test_context
    .new_command()
    .args("cache --quiet --node-modules-dir=none --vendor main.ts")
    .run();
  assert!(!node_modules_dir.exists());
}

#[test]
fn top_level_install_package_json_explicit_opt_in() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();
  let node_modules_dir = temp_dir.path().join("node_modules");
  let rm_created_files = || {
    std::fs::remove_dir_all(&node_modules_dir).unwrap();
    std::fs::remove_file(temp_dir.path().join("deno.lock")).unwrap();
  };

  // when the node_modules_dir is explicitly opted into, we should always
  // ensure a top level package.json install occurs
  temp_dir.write("deno.json", "{ \"nodeModulesDir\": \"auto\" }");
  temp_dir.write(
    "package.json",
    "{ \"dependencies\": { \"@denotest/esm-basic\": \"1.0\" }}",
  );

  temp_dir.write("main.ts", "console.log(5);");
  let output = test_context.new_command().args("cache main.ts").run();
  output.assert_matches_text(concat!(
    "Download http://localhost:4260/@denotest%2fesm-basic\n",
    "Download http://localhost:4260/@denotest/esm-basic/1.0.0.tgz\n",
    "Initialize @denotest/esm-basic@1.0.0\n",
  ));

  rm_created_files();
  let output = test_context
    .new_command()
    .args_vec(["eval", "console.log(5)"])
    .run();
  output.assert_matches_text(concat!(
    "Initialize @denotest/esm-basic@1.0.0\n",
    "5\n"
  ));

  rm_created_files();
  let output = test_context
    .new_command()
    .args("run -")
    .stdin_text("console.log(5)")
    .run();
  output.assert_matches_text(concat!(
    "Initialize @denotest/esm-basic@1.0.0\n",
    "5\n"
  ));

  // now ensure this is cached in the lsp
  rm_created_files();
  let mut client = test_context.new_lsp_command().build();
  client.initialize_default();
  let file_uri = temp_dir.url().join("file.ts").unwrap();
  client.did_open(json!({
    "textDocument": {
      "uri": file_uri,
      "languageId": "typescript",
      "version": 1,
      "text": "",
    }
  }));
  client.write_request(
    "workspace/executeCommand",
    json!({
      "command": "deno.cache",
      "arguments": [[], file_uri],
    }),
  );

  assert!(node_modules_dir.join("@denotest").exists());
}

#[test]
fn byonm_cjs_esm_packages() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();

  test_context.run_npm("init -y");
  test_context.run_npm("install @denotest/esm-basic @denotest/cjs-default-export @denotest/dual-cjs-esm chalk@4 chai@4.3");

  dir.write(
    "main.ts",
    r#"
import { getValue, setValue } from "@denotest/esm-basic";

setValue(2);
console.log(getValue());

import cjsDefault from "@denotest/cjs-default-export";
console.log(cjsDefault.default());
console.log(cjsDefault.named());

import { getKind } from "@denotest/dual-cjs-esm";
console.log(getKind());


"#,
  );
  let output = test_context.new_command().args("run --check main.ts").run();
  output
    .assert_matches_text("Check file:///[WILDCARD]/main.ts\n2\n1\n2\nesm\n");

  // should not have created the .deno directory
  assert!(!dir.path().join("node_modules/.deno").exists());

  // try chai
  dir.write(
    "chai.ts",
    r#"import { expect } from "chai";

    const timeout = setTimeout(() => {}, 0);
    expect(timeout).to.be.a("number");
    clearTimeout(timeout);"#,
  );
  test_context.new_command().args("run chai.ts").run();

  // try chalk cjs
  dir.write(
    "chalk.ts",
    "import chalk from 'chalk'; console.log(chalk.green('chalk cjs loads'));",
  );
  let output = test_context
    .new_command()
    .args("run --allow-read chalk.ts")
    .run();
  output.assert_matches_text("chalk cjs loads\n");

  // try using an npm specifier for chalk that matches the version we installed
  dir.write(
    "chalk.ts",
    "import chalk from 'npm:chalk@4'; console.log(chalk.green('chalk cjs loads'));",
  );
  let output = test_context
    .new_command()
    .args("run --allow-read chalk.ts")
    .run();
  output.assert_matches_text("chalk cjs loads\n");

  // try with one that doesn't match the package.json
  dir.write(
    "chalk.ts",
    "import chalk from 'npm:chalk@5'; console.log(chalk.green('chalk cjs loads'));",
  );
  let output = test_context
    .new_command()
    .args("run --allow-read chalk.ts")
    .run();
  output.assert_matches_text(
    r#"error: Could not find a matching package for 'npm:chalk@5' in the node_modules directory. Ensure you have all your JSR and npm dependencies listed in your deno.json or package.json, then run `deno install`. Alternatively, turn on auto-install by specifying `"nodeModulesDir": "auto"` in your deno.json file.
    at file:///[WILDCARD]chalk.ts:1:19
"#);
  output.assert_exit_code(1);
}

#[test]
fn future_byonm_cjs_esm_packages() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();

  test_context.run_npm("init -y");
  test_context.run_npm("install @denotest/esm-basic @denotest/cjs-default-export @denotest/dual-cjs-esm chalk@4 chai@4.3");

  dir.write(
    "main.ts",
    r#"
import { getValue, setValue } from "@denotest/esm-basic";

setValue(2);
console.log(getValue());

import cjsDefault from "@denotest/cjs-default-export";
console.log(cjsDefault.default());
console.log(cjsDefault.named());

import { getKind } from "@denotest/dual-cjs-esm";
console.log(getKind());


"#,
  );
  let output = test_context.new_command().args("run --check main.ts").run();
  output
    .assert_matches_text("Check file:///[WILDCARD]/main.ts\n2\n1\n2\nesm\n");

  // should not have created the .deno directory
  assert!(!dir.path().join("node_modules/.deno").exists());

  // try chai
  dir.write(
    "chai.ts",
    r#"import { expect } from "chai";

    const timeout = setTimeout(() => {}, 0);
    expect(timeout).to.be.a("number");
    clearTimeout(timeout);"#,
  );
  test_context.new_command().args("run chai.ts").run();

  // try chalk cjs
  dir.write(
    "chalk.ts",
    "import chalk from 'chalk'; console.log(chalk.green('chalk cjs loads'));",
  );
  let output = test_context
    .new_command()
    .args("run --allow-read chalk.ts")
    .run();
  output.assert_matches_text("chalk cjs loads\n");

  // try using an npm specifier for chalk that matches the version we installed
  dir.write(
    "chalk.ts",
    "import chalk from 'npm:chalk@4'; console.log(chalk.green('chalk cjs loads'));",
  );
  let output = test_context
    .new_command()
    .args("run --allow-read chalk.ts")
    .run();
  output.assert_matches_text("chalk cjs loads\n");

  // try with one that doesn't match the package.json
  dir.write(
    "chalk.ts",
    "import chalk from 'npm:chalk@5'; console.log(chalk.green('chalk cjs loads'));",
  );
  let output = test_context
    .new_command()
    .args("run --allow-read chalk.ts")
    .run();
  output.assert_matches_text(
    r#"error: Could not find a matching package for 'npm:chalk@5' in the node_modules directory. Ensure you have all your JSR and npm dependencies listed in your deno.json or package.json, then run `deno install`. Alternatively, turn on auto-install by specifying `"nodeModulesDir": "auto"` in your deno.json file.
    at file:///[WILDCARD]chalk.ts:1:19
"#);
  output.assert_exit_code(1);
}

#[test]
fn node_modules_dir_manual_import_map() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.write(
    "deno.json",
    r#"{
    "nodeModulesDir": "manual",
    "imports": {
      "basic": "npm:@denotest/esm-basic"
    }
}"#,
  );
  dir.write(
    "package.json",
    r#"{
    "name": "my-project",
    "version": "1.0.0",
    "type": "module",
    "dependencies": {
      "@denotest/esm-basic": "^1.0"
    }
}"#,
  );
  test_context.run_npm("install");

  dir.write(
    "main.ts",
    r#"
// import map should resolve
import { getValue } from "basic";
// and resolving via node resolution
import { setValue } from "@denotest/esm-basic";

setValue(5);
console.log(getValue());
"#,
  );
  let output = test_context.new_command().args("run main.ts").run();
  output.assert_matches_text("5\n");
  let output = test_context.new_command().args("check main.ts").run();
  output.assert_matches_text("Check file:///[WILDCARD]/main.ts\n");
}

#[test]
fn byonm_package_specifier_not_installed_and_invalid_subpath() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.path().join("package.json").write_json(&json!({
    "dependencies": {
      "chalk": "4",
      "@denotest/conditional-exports-strict": "1"
    }
  }));
  dir.write(
    "main.ts",
    "import chalk from 'chalk'; console.log(chalk.green('hi'));",
  );

  // no npm install has been run, so this should give an informative error
  let output = test_context.new_command().args("run main.ts").run();
  output.assert_matches_text(
    r#"error: Could not resolve "chalk", but found it in a package.json. Deno expects the node_modules/ directory to be up to date. Did you forget to run `deno install`?
    at file:///[WILDCARD]/main.ts:1:19
"#,
  );
  output.assert_exit_code(1);

  // now test for an invalid sub path after doing an npm install
  dir.write(
    "main.ts",
    "import '@denotest/conditional-exports-strict/test';",
  );

  test_context.run_npm("install");

  let output = test_context.new_command().args("run main.ts").run();
  output.assert_matches_text(
    r#"error: [ERR_PACKAGE_PATH_NOT_EXPORTED] Package subpath './test' is not defined by "exports" in '[WILDCARD]' imported from '[WILDCARD]main.ts'
    at file:///[WILDCARD]/main.ts:1:8
"#,
  );
  output.assert_exit_code(1);
}

#[test]
fn byonm_npm_workspaces() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.write("deno.json", r#"{ "unstable": [ "byonm" ] }"#);

  dir.write(
    "package.json",
    r#"{
  "name": "my-workspace",
  "workspaces": [
    "project-a",
    "project-b"
  ]
}
"#,
  );

  let project_a_dir = dir.path().join("project-a");
  project_a_dir.create_dir_all();
  project_a_dir.join("package.json").write_json(&json!({
    "name": "project-a",
    "version": "1.0.0",
    "main": "./index.js",
    "type": "module",
    "dependencies": {
      "chai": "^4.2",
      "project-b": "^1"
    }
  }));
  project_a_dir.join("index.js").write(
    r#"
import { expect } from "chai";

const timeout = setTimeout(() => {}, 0);
expect(timeout).to.be.a("number");
clearTimeout(timeout);

export function add(a, b) {
  return a + b;
}
"#,
  );
  project_a_dir
    .join("index.d.ts")
    .write("export function add(a: number, b: number): number;");

  let project_b_dir = dir.path().join("project-b");
  project_b_dir.create_dir_all();
  project_b_dir.join("package.json").write_json(&json!({
    "name": "project-b",
    "version": "1.0.0",
    "type": "module",
    "dependencies": {
      "@denotest/esm-basic": "^1.0",
    }
  }));
  project_b_dir.join("main.ts").write(
    r#"
import { getValue, setValue } from "@denotest/esm-basic";

setValue(5);
console.log(getValue());

import { add } from "project-a";
console.log(add(1, 2));
"#,
  );

  test_context.run_npm("install");

  let output = test_context
    .new_command()
    .args("run ./project-b/main.ts")
    .run();
  output.assert_matches_text("5\n3\n");
  let output = test_context
    .new_command()
    .args("check ./project-b/main.ts")
    .run();
  output.assert_matches_text("Check file:///[WILDCARD]/project-b/main.ts\n");

  // Now a file in the main directory should just be able to
  // import it via node resolution even though a package.json
  // doesn't exist here
  dir.write(
    "main.ts",
    r#"
import { getValue, setValue } from "@denotest/esm-basic";

setValue(7);
console.log(getValue());
"#,
  );
  let output = test_context.new_command().args("run main.ts").run();
  output.assert_matches_text("7\n");
  let output = test_context.new_command().args("check main.ts").run();
  output.assert_matches_text("Check file:///[WILDCARD]/main.ts\n");
}

#[test]
fn future_byonm_npm_workspaces() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();

  dir.write(
    "package.json",
    r#"{
  "name": "my-workspace",
  "workspaces": [
    "project-a",
    "project-b"
  ]
}
"#,
  );

  let project_a_dir = dir.path().join("project-a");
  project_a_dir.create_dir_all();
  project_a_dir.join("package.json").write_json(&json!({
    "name": "project-a",
    "version": "1.0.0",
    "main": "./index.js",
    "type": "module",
    "dependencies": {
      "chai": "^4.2",
      "project-b": "^1"
    }
  }));
  project_a_dir.join("index.js").write(
    r#"
import { expect } from "chai";

const timeout = setTimeout(() => {}, 0);
expect(timeout).to.be.a("number");
clearTimeout(timeout);

export function add(a, b) {
  return a + b;
}
"#,
  );
  project_a_dir
    .join("index.d.ts")
    .write("export function add(a: number, b: number): number;");

  let project_b_dir = dir.path().join("project-b");
  project_b_dir.create_dir_all();
  project_b_dir.join("package.json").write_json(&json!({
    "name": "project-b",
    "version": "1.0.0",
    "type": "module",
    "dependencies": {
      "@denotest/esm-basic": "^1.0",
    }
  }));
  project_b_dir.join("main.ts").write(
    r#"
import { getValue, setValue } from "@denotest/esm-basic";

setValue(5);
console.log(getValue());

import { add } from "project-a";
console.log(add(1, 2));
"#,
  );

  test_context.run_npm("install");

  let output = test_context
    .new_command()
    .args("run ./project-b/main.ts")
    .run();
  output.assert_matches_text("5\n3\n");
  let output = test_context
    .new_command()
    .args("check ./project-b/main.ts")
    .run();
  output.assert_matches_text("Check file:///[WILDCARD]/project-b/main.ts\n");

  // Now a file in the main directory should just be able to
  // import it via node resolution even though a package.json
  // doesn't exist here
  dir.write(
    "main.ts",
    r#"
import { getValue, setValue } from "@denotest/esm-basic";

setValue(7);
console.log(getValue());
"#,
  );
  let output = test_context.new_command().args("run main.ts").run();
  output.assert_matches_text("7\n");
  let output = test_context.new_command().args("check main.ts").run();
  output.assert_matches_text("Check file:///[WILDCARD]/main.ts\n");
}

#[test]
fn check_css_package_json_exports() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.write(
    "main.ts",
    r#"import "npm:@denotest/css-export/dist/index.css";"#,
  );
  test_context
    .new_command()
    .args("check main.ts")
    .run()
    .assert_matches_text("Download [WILDCARD]css-export\nDownload [WILDCARD]css-export/1.0.0.tgz\nCheck [WILDCARD]/main.ts\n")
    .assert_exit_code(0);
}

#[test]
fn cjs_export_analysis_require_re_export() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.write("deno.json", r#"{ "unstable": [ "byonm" ] }"#);

  dir.write(
    "package.json",
    r#"{ "name": "test", "packages": { "my-package": "1.0.0" } }"#,
  );
  dir.write(
    "main.js",
    "import { value1, value2 } from 'my-package';\nconsole.log(value1);\nconsole.log(value2)\n",
  );

  let node_modules_dir = dir.path().join("node_modules");

  // create a package at node_modules/.multipart/name/nested without a package.json
  {
    let pkg_dir = node_modules_dir
      .join(".multipart")
      .join("name")
      .join("nested");
    pkg_dir.create_dir_all();
    pkg_dir.join("index.js").write("module.exports.value1 = 5;");
  }
  // create a package at node_modules/.multipart/other with a package.json
  {
    let pkg_dir = node_modules_dir.join(".multipart").join("other");
    pkg_dir.create_dir_all();
    pkg_dir.join("index.js").write("module.exports.value2 = 6;");
  }
  // create a package at node_modules/my-package that requires them both
  {
    let pkg_dir = node_modules_dir.join("my-package");
    pkg_dir.create_dir_all();
    pkg_dir.join("package.json").write_json(&json!({
      "name": "my-package",
      "version": "1.0.0",
    }));
    pkg_dir
    .join("index.js")
    .write("module.exports = { ...require('.multipart/name/nested/index'), ...require('.multipart/other/index.js') }");
  }

  // the cjs export analysis was previously failing, but it should
  // resolve these exports similar to require
  let output = test_context
    .new_command()
    .args("run --allow-read main.js")
    .run();
  output.assert_matches_text("5\n6\n");
}

#[test]
fn cjs_rexport_analysis_json() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.write("deno.json", r#"{ "unstable": [ "byonm" ] }"#);

  dir.write("package.json", r#"{ "name": "test" }"#);
  dir.write(
    "main.js",
    "import data from 'my-package';\nconsole.log(data);\n",
  );

  let node_modules_dir = dir.path().join("node_modules");

  // create a package that has a json file at index.json and data.json then folder/index.json
  {
    let pkg_dir = node_modules_dir.join("data-package");
    pkg_dir.create_dir_all();
    pkg_dir.join("package.json").write_json(&json!({
      "name": "data-package",
      "version": "1.0.0",
    }));
    pkg_dir.join("index.json").write(r#"{ "value": 2 }"#);
    pkg_dir.join("data.json").write(r#"{ "value": 3 }"#);
    let folder = pkg_dir.join("folder");
    folder.create_dir_all();
    folder.join("index.json").write(r#"{ "value": 4 }"#);
  }
  // create a package at node_modules/my-package that re-exports a json file
  {
    let pkg_dir = node_modules_dir.join("my-package");
    pkg_dir.create_dir_all();
    pkg_dir.join("package.json").write_json(&json!({
      "name": "my-package",
      "version": "1.0.0",
    }));
    pkg_dir.join("data.json").write(r#"{ "value": 1 }"#);
    pkg_dir.join("index.js").write(
      "module.exports = {
  data1: require('./data'),
  data2: require('data-package'),
  data3: require('data-package/data'),
  data4: require('data-package/folder'),
};",
    );
  }

  let output = test_context
    .new_command()
    .args("run --allow-read main.js")
    .run();
  output.assert_matches_text(
    "{
  data1: { value: 1 },
  data2: { value: 2 },
  data3: { value: 3 },
  data4: { value: 4 }
}
",
  );
}

#[test]
fn cjs_export_analysis_import_cjs_directly_relative_import() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let dir = test_context.temp_dir();
  dir.write("deno.json", r#"{ "unstable": [ "byonm" ] }"#);

  dir.write(
    "package.json",
    r#"{ "name": "test", "dependencies": { "@denotest/cjs-default-export": "1.0.0" } }"#,
  );
  // previously it wasn't doing cjs export analysis on this file
  dir.write(
    "main.ts",
    "import { named } from './node_modules/@denotest/cjs-default-export/index.js';\nconsole.log(named());\n",
  );

  test_context.run_npm("install");

  let output = test_context.new_command().args("run main.ts").run();
  output.assert_matches_text("2\n");
}

#[test]
fn different_nested_dep_byonm() {
  let test_context = TestContextBuilder::for_npm()
    .use_copy_temp_dir("npm/different_nested_dep")
    .cwd("npm/different_nested_dep/")
    .build();

  test_context.run_npm("install");

  let output = test_context
    .new_command()
    .args("run --unstable-byonm main.js")
    .run();
  output.assert_matches_file("npm/different_nested_dep/main.out");
}

#[test]
fn different_nested_dep_byonm_future() {
  let test_context = TestContextBuilder::for_npm()
    .use_copy_temp_dir("npm/different_nested_dep")
    .cwd("npm/different_nested_dep/")
    .build();

  test_context.run_npm("install");

  let output = test_context.new_command().args("run main.js").run();
  output.assert_matches_file("npm/different_nested_dep/main.out");
}

#[test]
fn run_cjs_in_node_modules_folder() {
  let test_context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();
  temp_dir.write("package.json", "{}");
  temp_dir.write("deno.json", r#"{ "unstable": ["byonm"] }"#);
  let pkg_dir = temp_dir.path().join("node_modules/package");
  pkg_dir.create_dir_all();
  pkg_dir
    .join("package.json")
    .write(r#"{ "name": "package" }"#);
  pkg_dir
    .join("main.js")
    .write("console.log('hi'); module.exports = 'hi';");
  test_context
    .new_command()
    .args("run node_modules/package/main.js")
    .run()
    .assert_matches_text("hi\n");
}

#[tokio::test]
async fn test_private_npm_registry() {
  let _server = http_server();

  // For now just check that private server rejects requests without proper
  // auth header.

  let client = reqwest::Client::new();

  let url = Url::parse("http://127.0.0.1:4261/@denotest/basic").unwrap();

  let req = reqwest::Request::new(reqwest::Method::GET, url.clone());
  let resp = client.execute(req).await.unwrap();
  assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);

  let mut req = reqwest::Request::new(reqwest::Method::GET, url.clone());
  req.headers_mut().insert(
    reqwest::header::AUTHORIZATION,
    reqwest::header::HeaderValue::from_static("Bearer private-reg-token"),
  );
  let resp = client.execute(req).await.unwrap();
  assert_eq!(resp.status(), reqwest::StatusCode::OK);
}
