// Copyright 2018-2026 the Deno authors. MIT license.

use serde_json::json;
use test_util::TestContextBuilder;
use test_util::assert_contains;
use test_util::env_vars_for_jsr_npm_tests;
use test_util::pty::Pty;
use test_util::test;

#[test]
fn add_basic() {
  let starting_deno_json = json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
  });
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&starting_deno_json);

  let output = context.new_command().args("add jsr:@denotest/add").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
    "imports": {
      "@denotest/add": "jsr:@denotest/add@^1.0.0"
    }
  }));
}

#[test]
fn add_basic_no_deno_json() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context.new_command().args("add jsr:@denotest/add").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  // Don't use `assert_matches_json` to ensure the file is properly formatted.
  let expected = r#"{
  "imports": {
    "@denotest/add": "jsr:@denotest/add@^1.0.0"
  }
}
"#;
  temp_dir.join("deno.json").assert_matches_text(expected);
}

#[test]
fn add_basic_with_empty_deno_json() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", "");

  let output = context.new_command().args("add jsr:@denotest/add").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir
    .path()
    .join("deno.json")
    .assert_matches_json(json!({
      "imports": {
        "@denotest/add": "jsr:@denotest/add@^1.0.0"
      }
    }));
}

#[test]
fn add_version_contraint() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context.new_command().args("add jsr:@denotest/add@1").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/add": "jsr:@denotest/add@^1.0.0"
    }
  }));
}

#[test]
fn add_tilde() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context.new_command().args("add jsr:@denotest/add@~1").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/add": "jsr:@denotest/add@~1.0.0"
    }
  }));
}

#[test]
fn add_save_exact() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context
    .new_command()
    .args("add jsr:@denotest/add --save-exact")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/add": "jsr:@denotest/add@1.0.0"
    }
  }));
}

#[test]
fn add_exact_alias() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context
    .new_command()
    .args("add jsr:@denotest/add --exact")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/add": "jsr:@denotest/add@1.0.0"
    }
  }));
}

#[test]
fn add_multiple() {
  let starting_deno_json = json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
  });
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&starting_deno_json);

  let output = context
    .new_command()
    .args("add jsr:@denotest/add jsr:@denotest/subset-type-graph")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
    "imports": {
      "@denotest/add": "jsr:@denotest/add@^1.0.0",
      "@denotest/subset-type-graph": "jsr:@denotest/subset-type-graph@^0.1.0"
    }
  }));
}

#[test]
fn add_npm() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context.new_command().args("add npm:chalk@4.1").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add npm:chalk");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "chalk": "npm:chalk@^4.1.2"
    }
  }));
}

#[test]
fn add_npm_latest_default_minimum_dependency_age_downgrades() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context
    .new_command()
    .env_remove("NPM_CONFIG_MIN_RELEASE_AGE")
    .args("add npm:@denotest/min-release-age-latest@latest")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add npm:@denotest/min-release-age-latest@1.0.0");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/min-release-age-latest": "npm:@denotest/min-release-age-latest@^1.0.0"
    }
  }));
}

#[test]
fn add_npm_latest_minimum_dependency_age_downgrades() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "minimumDependencyAge": "2025-01-01T00:00:00.000Z",
  }));

  let output = context
    .new_command()
    .args("add npm:@denotest/min-release-age-latest@latest")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add npm:@denotest/min-release-age-latest@1.0.0");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "minimumDependencyAge": "2025-01-01T00:00:00.000Z",
    "imports": {
      "@denotest/min-release-age-latest": "npm:@denotest/min-release-age-latest@^1.0.0"
    }
  }));
}

#[test]
fn add_npm_latest_minimum_dependency_age_disabled() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "minimumDependencyAge": false,
  }));

  let output = context
    .new_command()
    .args("add npm:@denotest/min-release-age-latest@latest")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add npm:@denotest/min-release-age-latest@2.0.0");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "minimumDependencyAge": false,
    "imports": {
      "@denotest/min-release-age-latest": "npm:@denotest/min-release-age-latest@^2.0.0"
    }
  }));
}

#[test]
fn add_npm_latest_npmrc_min_release_age_downgrades() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join(".npmrc").write("min-release-age=1");

  let output = context
    .new_command()
    .args("add npm:@denotest/min-release-age-latest@latest")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add npm:@denotest/min-release-age-latest@1.0.0");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/min-release-age-latest": "npm:@denotest/min-release-age-latest@^1.0.0"
    }
  }));
}

#[test]
fn add_npm_minimum_dependency_age_no_matching_shows_hint() {
  let context = pm_context_builder().build();

  // 2.0.0 of this package has a publish date far in the future, so it is
  // always newer than the minimum dependency age. Requesting it specifically
  // leaves no installable version, and the error should point the user at the
  // "minimumDependencyAge" setting.
  let output = context
    .new_command()
    .env_remove("NPM_CONFIG_MIN_RELEASE_AGE")
    .args("add npm:@denotest/min-release-age-latest@2.0.0")
    .run();
  output.assert_exit_code(1);
  let output = output.combined_output();
  assert_contains!(output, "minimum dependency date");
  assert_contains!(output, "minimumDependencyAge");
}

fn pm_context_builder() -> TestContextBuilder {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_jsr_npm_tests())
    .use_temp_cwd()
}

#[test(flaky)]
fn approve_scripts_basic() {
  if !Pty::is_supported() {
    return;
  }
  let context = pm_context_builder().build();
  context
    .temp_dir()
    .write("deno.json", r#"{"nodeModulesDir": "manual"}"#);
  context
    .new_command()
    .args("install npm:@denotest/node-lifecycle-scripts@1.0.0")
    .run()
    .skip_output_check();
  context
    .new_command()
    .args("approve-scripts")
    .with_pty(|mut pty| {
      pty.expect("Select which packages to approve lifecycle scripts for");
      pty.expect("@denotest/node-lifecycle-scripts@1.0.0");
      pty.write_line(" ");
      pty.write_line("\r\n");
      pty.expect("Approved npm:@denotest/node-lifecycle-scripts@1.0.0");
      pty.expect("Ran build script npm:@denotest/node-lifecycle-scripts@1.0.0");
    });
  context
    .temp_dir()
    .path()
    .join("deno.json")
    .assert_matches_json(json!({
      "nodeModulesDir": "manual",
      "imports": {
        "@denotest/node-lifecycle-scripts": "npm:@denotest/node-lifecycle-scripts@1.0.0"
      },
      "allowScripts": ["npm:@denotest/node-lifecycle-scripts@1.0.0"],
    }));
  context
    .temp_dir()
    .path()
    .join("install.txt")
    .assert_matches_text("Installed by @denotest/node-lifecycle-scripts!");
}

#[test(flaky)]
fn approve_scripts_deny_some() {
  if !Pty::is_supported() {
    return;
  }
  let context = pm_context_builder().build();
  context
    .temp_dir()
    .write("deno.json", r#"{"nodeModulesDir": "manual"}"#);
  context
    .new_command()
    .args("install npm:@denotest/node-lifecycle-scripts@1.0.0 npm:@denotest/print-npm-user-agent@1.0.0")
    .run()
    .skip_output_check();
  context
    .new_command()
    .args("approve-scripts")
    .with_pty(|mut pty| {
      pty.expect("Select which packages to approve lifecycle scripts for");
      pty.expect("@denotest/node-lifecycle-scripts@1.0.0");
      pty.expect("@denotest/print-npm-user-agent@1.0.0");
      pty.write_line(" ");
      pty.write_line("\r\n");
      pty.expect("Denied npm:@denotest/print-npm-user-agent@1.0.0");
      pty.expect("Approved npm:@denotest/node-lifecycle-scripts@1.0.0");
      pty.expect("Ran build script npm:@denotest/node-lifecycle-scripts@1.0.0");
    });
  context.temp_dir().path().join("deno.json").assert_matches_json(json!({
    "nodeModulesDir": "manual",
    "imports": {
      "@denotest/node-lifecycle-scripts": "npm:@denotest/node-lifecycle-scripts@1.0.0",
      "@denotest/print-npm-user-agent": "npm:@denotest/print-npm-user-agent@1.0.0"
    },
    "allowScripts": {
      "allow": ["npm:@denotest/node-lifecycle-scripts@1.0.0"],
      "deny": ["npm:@denotest/print-npm-user-agent@1.0.0"]
    },
  }));
  context
    .temp_dir()
    .path()
    .join("install.txt")
    .assert_matches_text("Installed by @denotest/node-lifecycle-scripts!");
}

#[test(flaky)]
fn approve_scripts_no_lock_explicit_package() {
  let context = pm_context_builder().build();
  context
    .temp_dir()
    .write("deno.json", r#"{"lock": false, "nodeModulesDir": "auto"}"#);
  context.temp_dir().write(
    "package.json",
    r#"{"dependencies": {"@denotest/node-lifecycle-scripts": "*"}}"#,
  );
  context
    .new_command()
    .args("install")
    .run()
    .skip_output_check();
  context
    .new_command()
    .args("approve-scripts npm:@denotest/node-lifecycle-scripts")
    .run()
    .assert_matches_text(
      "[WILDCARD]Approved npm:@denotest/node-lifecycle-scripts[WILDCARD]",
    );
}

#[test(flaky)]
fn approve_scripts_no_lock_detects_packages() {
  if !Pty::is_supported() {
    return;
  }
  let context = pm_context_builder().build();
  context
    .temp_dir()
    .write("deno.json", r#"{"lock": false, "nodeModulesDir": "auto"}"#);
  context.temp_dir().write(
    "package.json",
    r#"{"dependencies": {"@denotest/node-lifecycle-scripts": "*"}}"#,
  );
  context
    .new_command()
    .args("install")
    .run()
    .skip_output_check();
  // Without explicit package args, approve-scripts should detect packages
  // with lifecycle scripts and show the interactive picker.
  context
    .new_command()
    .args("approve-scripts")
    .with_pty(|mut pty| {
      pty.expect("Select which packages to approve lifecycle scripts for");
      pty.expect("@denotest/node-lifecycle-scripts@1.0.0");
      pty.write_line(" ");
      pty.write_line("\r\n");
      pty.expect("Approved npm:@denotest/node-lifecycle-scripts@1.0.0");
    });
}

#[test(flaky)]
fn update_interactive_shows_version_req() {
  if !Pty::is_supported() {
    return;
  }
  let context = pm_context_builder().build();
  // The requirement (`^1.0.0`) resolves to the latest compatible version
  // (`1.0.1`). The interactive picker must show the version requirement being
  // updated (`1.0.0 -> 1.0.1`), not the resolved version on both sides
  // (`1.0.1 -> 1.0.1`). Regression test for
  // https://github.com/denoland/deno/issues/34668
  context.temp_dir().write(
    "deno.json",
    r#"{"imports": {"@denotest/update-latest-semver": "npm:@denotest/update-latest-semver@^1.0.0"}}"#,
  );
  context
    .new_command()
    .args("install")
    .run()
    .skip_output_check();
  context
    .new_command()
    .args("update --interactive")
    .with_pty(|mut pty| {
      pty.expect("Select which packages to update");
      pty.expect("1.0.0 -> 1.0.1");
    });
}
