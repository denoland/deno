// Copyright 2018-2025 the Deno authors. MIT license.

use deno_cache_dir::HttpCache;
use deno_lockfile::Lockfile;
use deno_lockfile::NewLockfileOptions;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageNv;
use serde_json::json;
use serde_json::Value;
use test_util as util;
use url::Url;
use util::assert_contains;
use util::assert_not_contains;
use util::TestContextBuilder;

#[test]
fn fast_check_cache() {
  let test_context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let deno_dir = test_context.deno_dir();
  let temp_dir = test_context.temp_dir();
  let type_check_cache_path = deno_dir.path().join("check_cache_v2");

  temp_dir.write(
    "main.ts",
    r#"import { add } from "jsr:@denotest/add@1";
    const value: number = add(1, 2);
    console.log(value);"#,
  );
  temp_dir.path().join("deno.json").write_json(&json!({
    "vendor": true
  }));

  test_context
    .new_command()
    .args("check main.ts")
    .run()
    .skip_output_check();

  type_check_cache_path.remove_file();
  let check_debug_cmd = test_context
    .new_command()
    .args("check --log-level=debug main.ts");
  let output = check_debug_cmd.run();
  assert_contains!(
    output.combined_output(),
    "Using FastCheck cache for: @denotest/add@1.0.0"
  );

  // modify the file in the vendor folder
  let vendor_dir = temp_dir.path().join("vendor");
  let pkg_dir = vendor_dir.join("http_127.0.0.1_4250/@denotest/add/1.0.0/");
  pkg_dir
    .join("mod.ts")
    .append("\nexport * from './other.ts';");
  let nested_pkg_file = pkg_dir.join("other.ts");
  nested_pkg_file.write("export function other(): string { return ''; }");

  // invalidated
  let output = check_debug_cmd.run();
  assert_not_contains!(
    output.combined_output(),
    "Using FastCheck cache for: @denotest/add@1.0.0"
  );

  // ensure cache works
  let output = check_debug_cmd.run();
  assert_contains!(output.combined_output(), "Already type checked");

  // now validated
  type_check_cache_path.remove_file();
  let output = check_debug_cmd.run();
  let building_fast_check_msg = "Building fast check graph";
  assert_contains!(output.combined_output(), building_fast_check_msg);
  assert_contains!(
    output.combined_output(),
    "Using FastCheck cache for: @denotest/add@1.0.0"
  );

  // cause a fast check error in the nested package
  nested_pkg_file
    .append("\nexport function asdf(a: number) { let err: number = ''; return Math.random(); }");
  check_debug_cmd.run().skip_output_check();

  // ensure the cache still picks it up for this file
  type_check_cache_path.remove_file();
  let output = check_debug_cmd.run();
  assert_contains!(output.combined_output(), building_fast_check_msg);
  assert_contains!(
    output.combined_output(),
    "Using FastCheck cache for: @denotest/add@1.0.0"
  );

  // see that the type checking error in the internal function gets surfaced with --all
  test_context
    .new_command()
    .args("check --all main.ts")
    .run()
    .assert_matches_text(
      "Check file:///[WILDCARD]main.ts
TS2322 [ERROR]: Type 'string' is not assignable to type 'number'.
export function asdf(a: number) { let err: number = ''; return Math.random(); }
                                      ~~~
    at http://127.0.0.1:4250/@denotest/add/1.0.0/other.ts:2:39

error: Type checking failed.
",
    )
    .assert_exit_code(1);

  // now fix the package
  nested_pkg_file.write("export function test() {}");
  let output = check_debug_cmd.run();
  assert_contains!(output.combined_output(), building_fast_check_msg);
  assert_not_contains!(
    output.combined_output(),
    "Using FastCheck cache for: @denotest/add@1.0.0"
  );

  // finally ensure it uses the cache
  type_check_cache_path.remove_file();
  let output = check_debug_cmd.run();
  assert_contains!(output.combined_output(), building_fast_check_msg);
  assert_contains!(
    output.combined_output(),
    "Using FastCheck cache for: @denotest/add@1.0.0"
  );
}

struct TestNpmPackageInfoProvider;

#[async_trait::async_trait(?Send)]
impl deno_lockfile::NpmPackageInfoProvider for TestNpmPackageInfoProvider {
  async fn get_npm_package_info(
    &self,
    values: &[deno_semver::package::PackageNv],
  ) -> Result<
    Vec<deno_lockfile::Lockfile5NpmInfo>,
    Box<dyn std::error::Error + Send + Sync>,
  > {
    Ok(values.iter().map(|_| Default::default()).collect())
  }
}

#[tokio::test]
async fn specifiers_in_lockfile() {
  let test_context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();

  temp_dir.write(
    "main.ts",
    r#"import version from "jsr:@denotest/no-module-graph@0.1";

console.log(version);"#,
  );
  temp_dir.write("deno.json", "{}"); // to automatically create a lockfile

  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text("0.1.1\n");

  let lockfile_path = temp_dir.path().join("deno.lock");
  let mut lockfile = Lockfile::new(
    NewLockfileOptions {
      file_path: lockfile_path.to_path_buf(),
      content: &lockfile_path.read_to_string(),
      overwrite: false,
    },
    &TestNpmPackageInfoProvider,
  )
  .await
  .unwrap();
  *lockfile
    .content
    .packages
    .specifiers
    .get_mut(
      &JsrDepPackageReq::from_str("jsr:@denotest/no-module-graph@0.1").unwrap(),
    )
    .unwrap() = "0.1.0".into();
  lockfile_path.write(lockfile.as_json_string());

  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text("0.1.0\n");
}

#[test]
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
    let specifier =
      Url::parse(&format!("http://127.0.0.1:4250/{}/meta.json", package))
        .unwrap();
    let cache = deno_cache_dir::GlobalHttpCache::new(
      sys_traits::impls::RealSys,
      deno_dir.path().join("remote").to_path_buf(),
    );
    let entry = cache
      .get(&cache.cache_item_key(&specifier).unwrap(), None)
      .unwrap()
      .unwrap();
    let mut registry_json: serde_json::Value =
      serde_json::from_slice(&entry.content).unwrap();
    remove_version(&mut registry_json, version);
    cache
      .set(
        &specifier,
        entry.metadata.headers.clone(),
        registry_json.to_string().as_bytes(),
      )
      .unwrap();
  }

  // This tests that when a local machine doesn't have a version
  // specified in a dependency that exists in the npm registry
  let test_context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let deno_dir = test_context.deno_dir();
  let temp_dir = test_context.temp_dir();
  temp_dir.write(
    "main.ts",
    "import { add } from 'jsr:@denotest/add@1'; console.log(add(1, 2));",
  );

  // cache successfully to the deno_dir
  let output = test_context.new_command().args("cache main.ts").run();
  output.assert_matches_text(concat!(
    "Download http://127.0.0.1:4250/@denotest/add/meta.json\n",
    "Download http://127.0.0.1:4250/@denotest/add/1.0.0_meta.json\n",
    "Download http://127.0.0.1:4250/@denotest/add/1.0.0/mod.ts\n",
  ));

  // modify the package information in the cache to remove the latest version
  remove_version_for_package(deno_dir, "@denotest/add", "1.0.0");

  // should error when `--cache-only` is used now because the version is not in the cache
  let output = test_context
    .new_command()
    .args("run --cached-only main.ts")
    .run();
  output.assert_exit_code(1);
  output.assert_matches_text("error: JSR package manifest for '@denotest/add' failed to load. Could not resolve version constraint using only cached data. Try running again without --cached-only
    at file:///[WILDCARD]main.ts:1:21
");

  // now try running without it, it should download the package now
  test_context
    .new_command()
    .args("run main.ts")
    .run()
    .assert_matches_text(concat!(
      "Download http://127.0.0.1:4250/@denotest/add/meta.json\n",
      "Download http://127.0.0.1:4250/@denotest/add/1.0.0_meta.json\n",
      "3\n",
    ))
    .assert_exit_code(0);
}

#[tokio::test]
async fn lockfile_bad_package_integrity() {
  let test_context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();

  temp_dir.write(
    "main.ts",
    r#"import version from "jsr:@denotest/no-module-graph@0.1";

console.log(version);"#,
  );
  temp_dir.write("deno.json", "{}"); // to automatically create a lockfile

  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text("0.1.1\n");

  let lockfile_path = temp_dir.path().join("deno.lock");
  let mut lockfile = Lockfile::new(
    NewLockfileOptions {
      file_path: lockfile_path.to_path_buf(),
      content: &lockfile_path.read_to_string(),
      overwrite: false,
    },
    &TestNpmPackageInfoProvider,
  )
  .await
  .unwrap();
  let pkg_nv = "@denotest/no-module-graph@0.1.1";
  let original_integrity = get_lockfile_pkg_integrity(&lockfile, pkg_nv);
  set_lockfile_pkg_integrity(&mut lockfile, pkg_nv, "bad_integrity");
  lockfile_path.write(lockfile.as_json_string());

  let actual_integrity =
    test_context.get_jsr_package_integrity("@denotest/no-module-graph/0.1.1");
  let integrity_check_failed_msg = format!("[WILDCARD]Integrity check failed for package. The source code is invalid, as it does not match the expected hash in the lock file.

  Package: @denotest/no-module-graph@0.1.1
  Actual: {}
  Expected: bad_integrity

This could be caused by:
  * the lock file may be corrupt
  * the source itself may be corrupt

Investigate the lockfile; delete it to regenerate the lockfile or --reload to reload the source code from the server.
", actual_integrity);
  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text(&integrity_check_failed_msg)
    .assert_exit_code(10);

  // now try with a vendor folder
  temp_dir
    .path()
    .join("deno.json")
    .write_json(&json!({ "vendor": true }));

  // should fail again
  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text(&integrity_check_failed_msg)
    .assert_exit_code(10);

  // now update to the correct integrity
  set_lockfile_pkg_integrity(&mut lockfile, pkg_nv, &original_integrity);
  lockfile_path.write(lockfile.as_json_string());

  // should pass now
  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text("0.1.1\n")
    .assert_exit_code(0);

  // now update to a bad integrity again
  set_lockfile_pkg_integrity(&mut lockfile, pkg_nv, "bad_integrity");
  lockfile_path.write(lockfile.as_json_string());

  // shouldn't matter because we have a vendor folder
  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text("0.1.1\n")
    .assert_exit_code(0);

  // now remove the vendor dir and it should fail again
  temp_dir.path().join("vendor").remove_dir_all();

  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text(&integrity_check_failed_msg)
    .assert_exit_code(10);
}

#[test]
fn bad_manifest_checksum() {
  let test_context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();

  temp_dir.write(
    "main.ts",
    r#"import { add } from "jsr:@denotest/bad-manifest-checksum@1.0.0";
console.log(add);"#,
  );

  // test it properly checks the checksum on download
  test_context
    .new_command()
    .args("run main.ts")
    .run()
    .assert_matches_text(
      "Download http://127.0.0.1:4250/@denotest/bad-manifest-checksum/meta.json
Download http://127.0.0.1:4250/@denotest/bad-manifest-checksum/1.0.0_meta.json
Download http://127.0.0.1:4250/@denotest/bad-manifest-checksum/1.0.0/mod.ts
error: Integrity check failed in package. The package may have been tampered with.

  Specifier: http://127.0.0.1:4250/@denotest/bad-manifest-checksum/1.0.0/mod.ts
  Actual: 9a30ac96b5d5c1b67eca69e1e2cf0798817d9578c8d7d904a81a67b983b35cba
  Expected: bad-checksum

If you modified your global cache, run again with the --reload flag to restore its state. If you want to modify dependencies locally run again with the --vendor flag or specify `\"vendor\": true` in a deno.json then modify the contents of the vendor/ folder.
",
    )
    .assert_exit_code(10);

  // test it properly checks the checksum when loading from the cache
  test_context
    .new_command()
    .args("run main.ts")
    .run()
    .assert_matches_text(
      "error: Integrity check failed in package. The package may have been tampered with.

  Specifier: http://127.0.0.1:4250/@denotest/bad-manifest-checksum/1.0.0/mod.ts
  Actual: 9a30ac96b5d5c1b67eca69e1e2cf0798817d9578c8d7d904a81a67b983b35cba
  Expected: bad-checksum

If you modified your global cache, run again with the --reload flag to restore its state. If you want to modify dependencies locally run again with the --vendor flag or specify `\"vendor\": true` in a deno.json then modify the contents of the vendor/ folder.
",
    )
    .assert_exit_code(10);
}

fn get_lockfile_pkg_integrity(lockfile: &Lockfile, pkg_nv: &str) -> String {
  lockfile
    .content
    .packages
    .jsr
    .get(&PackageNv::from_str(pkg_nv).unwrap())
    .unwrap()
    .integrity
    .clone()
}

fn set_lockfile_pkg_integrity(
  lockfile: &mut Lockfile,
  pkg_nv: &str,
  integrity: &str,
) {
  lockfile
    .content
    .packages
    .jsr
    .get_mut(&PackageNv::from_str(pkg_nv).unwrap())
    .unwrap()
    .integrity = integrity.to_string();
}
