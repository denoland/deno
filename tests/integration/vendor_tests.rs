// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json;
use deno_core::serde_json::json;
use pretty_assertions::assert_eq;
use std::fmt::Write as _;
use std::path::PathBuf;
use test_util as util;
use test_util::itest;
use test_util::TempDir;
use util::http_server;
use util::new_deno_dir;
use util::TestContextBuilder;

#[test]
fn output_dir_exists() {
  let t = TempDir::new();
  t.write("mod.ts", "");
  t.create_dir_all("vendor");
  t.write("vendor/mod.ts", "");

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("mod.ts")
    .stderr_piped()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    concat!(
      "error: Output directory was not empty. Please specify an empty ",
      "directory or use --force to ignore this error and potentially ",
      "overwrite its contents.",
    ),
  );
  assert!(!output.status.success());

  // ensure it errors when using the `--output` arg too
  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("--output")
    .arg("vendor")
    .arg("mod.ts")
    .stderr_piped()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    concat!(
      "error: Output directory was not empty. Please specify an empty ",
      "directory or use --force to ignore this error and potentially ",
      "overwrite its contents.",
    ),
  );
  assert!(!output.status.success());

  // now use `--force`
  let status = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("mod.ts")
    .arg("--force")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn standard_test() {
  let _server = http_server();
  let t = TempDir::new();
  let vendor_dir = t.path().join("vendor2");
  t.write(
    "my_app.ts",
    "import {Logger} from 'http://localhost:4545/vendor/query_reexport.ts?testing'; new Logger().log('outputted');",
  );

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("my_app.ts")
    .arg("--output")
    .arg("vendor2")
    .env("NO_COLOR", "1")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      concat!(
        "Download http://localhost:4545/vendor/query_reexport.ts?testing\n",
        "Download http://localhost:4545/vendor/logger.ts?test\n",
        "{}",
      ),
      success_text("2 modules", "vendor2", true),
    )
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
  assert!(output.status.success());

  assert!(vendor_dir.exists());
  assert!(!t.path().join("vendor").exists());
  let import_map: serde_json::Value =
    serde_json::from_str(&t.read_to_string("vendor2/import_map.json")).unwrap();
  assert_eq!(
    import_map,
    json!({
      "imports": {
        "http://localhost:4545/vendor/query_reexport.ts?testing": "./localhost_4545/vendor/query_reexport.ts",
        "http://localhost:4545/": "./localhost_4545/",
      },
      "scopes": {
        "./localhost_4545/": {
          "./localhost_4545/vendor/logger.ts?test": "./localhost_4545/vendor/logger.ts"
        }
      }
    }),
  );

  // try running the output with `--no-remote`
  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--no-remote")
    .arg("--check")
    .arg("--quiet")
    .arg("--import-map")
    .arg("vendor2/import_map.json")
    .arg("my_app.ts")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "");
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "outputted");
  assert!(output.status.success());
}

#[test]
fn import_map_output_dir() {
  let _server = http_server();
  let t = TempDir::new();
  t.write("mod.ts", "");
  t.create_dir_all("vendor");
  t.write(
    "vendor/import_map.json",
    // will be ignored
    "{ \"imports\": { \"https://localhost:4545/\": \"./localhost/\" }}",
  );
  t.write(
    "deno.json",
    "{ \"import_map\": \"./vendor/import_map.json\" }",
  );
  t.write(
    "my_app.ts",
    "import {Logger} from 'http://localhost:4545/vendor/logger.ts'; new Logger().log('outputted');",
  );

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("--force")
    .arg("--import-map")
    .arg("vendor/import_map.json")
    .arg("my_app.ts")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      concat!(
        "{}\n",
        "Download http://localhost:4545/vendor/logger.ts\n",
        "{}\n\n{}",
      ),
      ignoring_import_map_text(),
      vendored_text("1 module", "vendor/"),
      success_text_updated_deno_json("vendor/"),
    )
  );
  assert!(output.status.success());
}

#[test]
fn remote_module_test() {
  let _server = http_server();
  let t = TempDir::new();
  let vendor_dir = t.path().join("vendor");

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("http://localhost:4545/vendor/query_reexport.ts")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      concat!(
        "Download http://localhost:4545/vendor/query_reexport.ts\n",
        "Download http://localhost:4545/vendor/logger.ts?test\n",
        "{}",
      ),
      success_text("2 modules", "vendor/", true),
    )
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
  assert!(output.status.success());
  assert!(vendor_dir.exists());
  assert!(vendor_dir
    .join("localhost_4545/vendor/query_reexport.ts")
    .exists());
  assert!(vendor_dir.join("localhost_4545/vendor/logger.ts").exists());
  let import_map: serde_json::Value =
    serde_json::from_str(&t.read_to_string("vendor/import_map.json")).unwrap();
  assert_eq!(
    import_map,
    json!({
      "imports": {
        "http://localhost:4545/": "./localhost_4545/",
      },
      "scopes": {
        "./localhost_4545/": {
          "./localhost_4545/vendor/logger.ts?test": "./localhost_4545/vendor/logger.ts",
        }
      }
    }),
  );
}

#[test]
fn existing_import_map_no_remote() {
  let _server = http_server();
  let t = TempDir::new();
  t.write(
    "mod.ts",
    "import {Logger} from 'http://localhost:4545/vendor/logger.ts';",
  );
  let import_map_filename = "imports2.json";
  let import_map_text =
    r#"{ "imports": { "http://localhost:4545/vendor/": "./logger/" } }"#;
  t.write(import_map_filename, import_map_text);
  t.create_dir_all("logger");
  t.write("logger/logger.ts", "export class Logger {}");

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("mod.ts")
    .arg("--import-map")
    .arg(import_map_filename)
    .stderr_piped()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    success_text("0 modules", "vendor/", false)
  );
  assert!(output.status.success());
  // it should not have found any remote dependencies because
  // the provided import map mapped it to a local directory
  assert_eq!(t.read_to_string(import_map_filename), import_map_text);
}

#[test]
fn existing_import_map_mixed_with_remote() {
  let _server = http_server();
  let deno_dir = new_deno_dir();
  let t = TempDir::new();
  t.write(
    "mod.ts",
    "import {Logger} from 'http://localhost:4545/vendor/logger.ts';",
  );

  let status = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(t.path())
    .arg("vendor")
    .arg("mod.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());

  assert_eq!(
    t.read_to_string("vendor/import_map.json"),
    r#"{
  "imports": {
    "http://localhost:4545/": "./localhost_4545/"
  }
}
"#,
  );

  // make the import map specific to support vendoring mod.ts in the next step
  t.write(
    "vendor/import_map.json",
    r#"{
  "imports": {
    "http://localhost:4545/vendor/logger.ts": "./localhost_4545/vendor/logger.ts"
  }
}
"#,
  );

  t.write(
    "mod.ts",
    concat!(
      "import {Logger} from 'http://localhost:4545/vendor/logger.ts';\n",
      "import {Logger as OtherLogger} from 'http://localhost:4545/vendor/mod.ts';\n",
    ),
  );

  // now vendor with the existing import map in a separate vendor directory
  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .env("NO_COLOR", "1")
    .current_dir(t.path())
    .arg("vendor")
    .arg("mod.ts")
    .arg("--import-map")
    .arg("vendor/import_map.json")
    .arg("--output")
    .arg("vendor2")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      concat!("Download http://localhost:4545/vendor/mod.ts\n", "{}",),
      success_text("1 module", "vendor2", true),
    )
  );
  assert!(output.status.success());

  // tricky scenario here where the output directory now contains a mapping
  // back to the previous vendor location
  assert_eq!(
    t.read_to_string("vendor2/import_map.json"),
    r#"{
  "imports": {
    "http://localhost:4545/vendor/logger.ts": "../vendor/localhost_4545/vendor/logger.ts",
    "http://localhost:4545/": "./localhost_4545/"
  },
  "scopes": {
    "./localhost_4545/": {
      "./localhost_4545/vendor/logger.ts": "../vendor/localhost_4545/vendor/logger.ts"
    }
  }
}
"#,
  );

  // ensure it runs
  let status = util::deno_cmd()
    .current_dir(t.path())
    .arg("run")
    .arg("--check")
    .arg("--no-remote")
    .arg("--import-map")
    .arg("vendor2/import_map.json")
    .arg("mod.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn dynamic_import() {
  let _server = http_server();
  let t = TempDir::new();
  t.write(
    "mod.ts",
    "import {Logger} from 'http://localhost:4545/vendor/dynamic.ts'; new Logger().log('outputted');",
  );

  let status = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("mod.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let import_map: serde_json::Value =
    serde_json::from_str(&t.read_to_string("vendor/import_map.json")).unwrap();
  assert_eq!(
    import_map,
    json!({
      "imports": {
        "http://localhost:4545/": "./localhost_4545/",
      }
    }),
  );

  // try running the output with `--no-remote`
  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--allow-read=.")
    .arg("--no-remote")
    .arg("--check")
    .arg("--quiet")
    .arg("--import-map")
    .arg("vendor/import_map.json")
    .arg("mod.ts")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "");
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "outputted");
  assert!(output.status.success());
}

#[test]
fn dynamic_non_analyzable_import() {
  let _server = http_server();
  let t = TempDir::new();
  t.write(
    "mod.ts",
    "import {Logger} from 'http://localhost:4545/vendor/dynamic_non_analyzable.ts'; new Logger().log('outputted');",
  );

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("--reload")
    .arg("mod.ts")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  // todo(https://github.com/denoland/deno_graph/issues/138): it should warn about
  // how it couldn't analyze the dynamic import
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      "Download http://localhost:4545/vendor/dynamic_non_analyzable.ts\n{}",
      success_text("1 module", "vendor/", true),
    )
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
  assert!(output.status.success());
}

itest!(dynamic_non_existent {
  args: "vendor http://localhost:4545/vendor/dynamic_non_existent.ts",
  temp_cwd: true,
  exit_code: 0,
  http_server: true,
  output: "vendor/dynamic_non_existent.ts.out",
});

#[test]
fn update_existing_config_test() {
  let _server = http_server();
  let t = TempDir::new();
  t.write(
    "my_app.ts",
    "import {Logger} from 'http://localhost:4545/vendor/logger.ts'; new Logger().log('outputted');",
  );
  t.write("deno.json", "{\n}");

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("my_app.ts")
    .arg("--output")
    .arg("vendor2")
    .env("NO_COLOR", "1")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      "Download http://localhost:4545/vendor/logger.ts\n{}\n\n{}",
      vendored_text("1 module", "vendor2"),
      success_text_updated_deno_json("vendor2",)
    )
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
  assert!(output.status.success());

  // try running the output with `--no-remote` and not specifying a `--vendor`
  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--no-remote")
    .arg("--check")
    .arg("--quiet")
    .arg("my_app.ts")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "");
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "outputted");
  assert!(output.status.success());
}

#[test]
fn update_existing_empty_config_test() {
  let _server = http_server();
  let t = TempDir::new();
  t.write(
    "my_app.ts",
    "import {Logger} from 'http://localhost:4545/vendor/logger.ts'; new Logger().log('outputted');",
  );
  t.write("deno.json", "");

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("my_app.ts")
    .arg("--output")
    .arg("vendor2")
    .env("NO_COLOR", "1")
    .piped_output()
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      "Download http://localhost:4545/vendor/logger.ts\n{}\n\n{}",
      vendored_text("1 module", "vendor2"),
      success_text_updated_deno_json("vendor2",)
    )
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
  assert!(output.status.success());
}

#[test]
fn vendor_npm_node_specifiers() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "my_app.ts",
    concat!(
      "import { path, getValue, setValue } from 'http://localhost:4545/vendor/npm_and_node_specifier.ts';\n",
      "setValue(5);\n",
      "console.log(path.isAbsolute(Deno.cwd()), getValue());",
    ),
  );
  temp_dir.write("deno.json", "{}");

  let output = context.new_command().args("vendor my_app.ts").run();
  output.assert_matches_text(format!(
    concat!(
      "Download http://localhost:4545/vendor/npm_and_node_specifier.ts\n",
      "Download http://localhost:4260/@denotest/esm-basic\n",
      "Download http://localhost:4260/@denotest/esm-basic/1.0.0.tgz\n",
      "{}\n",
      "Initialize @denotest/esm-basic@1.0.0\n",
      "{}\n\n",
      "{}\n",
    ),
    vendored_text("1 module", "vendor/"),
    vendored_npm_package_text("1 npm package"),
    success_text_updated_deno_json("vendor/")
  ));
  let output = context.new_command().args("run -A my_app.ts").run();
  output.assert_matches_text("true 5\n");
  assert!(temp_dir.path().join("node_modules").exists());
  assert!(temp_dir.path().join("deno.lock").exists());

  // now try re-vendoring with a lockfile
  let output = context.new_command().args("vendor --force my_app.ts").run();
  output.assert_matches_text(format!(
    "{}\n{}\n\n{}\n",
    ignoring_import_map_text(),
    vendored_text("1 module", "vendor/"),
    success_text_updated_deno_json("vendor/"),
  ));

  // delete the node_modules folder
  temp_dir.remove_dir_all("node_modules");

  // vendor with --node-modules-dir=false
  let output = context
    .new_command()
    .args("vendor --node-modules-dir=false --force my_app.ts")
    .run();
  output.assert_matches_text(format!(
    "{}\n{}\n\n{}\n",
    ignoring_import_map_text(),
    vendored_text("1 module", "vendor/"),
    success_text_updated_deno_json("vendor/")
  ));
  assert!(!temp_dir.path().join("node_modules").exists());

  // delete the deno.json
  temp_dir.remove_file("deno.json");

  // vendor with --node-modules-dir
  let output = context
    .new_command()
    .args("vendor --node-modules-dir --force my_app.ts")
    .run();
  output.assert_matches_text(format!(
    "Initialize @denotest/esm-basic@1.0.0\n{}\n\n{}\n",
    vendored_text("1 module", "vendor/"),
    use_import_map_text("vendor/")
  ));
}

#[test]
fn vendor_only_npm_specifiers() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "my_app.ts",
    concat!(
      "import { getValue, setValue } from 'npm:@denotest/esm-basic';\n",
      "setValue(5);\n",
      "console.log(path.isAbsolute(Deno.cwd()), getValue());",
    ),
  );
  temp_dir.write("deno.json", "{}");

  let output = context.new_command().args("vendor my_app.ts").run();
  output.assert_matches_text(format!(
    concat!(
      "Download http://localhost:4260/@denotest/esm-basic\n",
      "Download http://localhost:4260/@denotest/esm-basic/1.0.0.tgz\n",
      "{}\n",
      "Initialize @denotest/esm-basic@1.0.0\n",
      "{}\n",
    ),
    vendored_text("0 modules", "vendor/"),
    vendored_npm_package_text("1 npm package"),
  ));
}

fn success_text(module_count: &str, dir: &str, has_import_map: bool) -> String {
  let mut text = format!("Vendored {module_count} into {dir} directory.");
  if has_import_map {
    write!(text, "\n\n{}", use_import_map_text(dir)).unwrap();
  }
  text
}

fn use_import_map_text(dir: &str) -> String {
  format!(
    concat!(
      "To use vendored modules, specify the `--import-map {}import_map.json` flag when ",
      r#"invoking Deno subcommands or add an `"importMap": "<path_to_vendored_import_map>"` "#,
      "entry to a deno.json file.",
    ),
    if dir != "vendor/" {
      format!("{}{}", dir.trim_end_matches('/'), if cfg!(windows) { '\\' } else {'/'})
    } else {
      dir.to_string()
    }
  )
}

fn vendored_text(module_count: &str, dir: &str) -> String {
  format!("Vendored {} into {} directory.", module_count, dir)
}

fn vendored_npm_package_text(package_count: &str) -> String {
  format!(
    concat!(
     "Vendored {} into node_modules directory. Set `nodeModulesDir: false` ",
     "in the Deno configuration file to disable vendoring npm packages in the future.",
    ),
    package_count
  )
}

fn success_text_updated_deno_json(dir: &str) -> String {
  format!(
    concat!(
      "Updated your local Deno configuration file with a reference to the ",
      "new vendored import map at {}import_map.json. Invoking Deno subcommands will ",
      "now automatically resolve using the vendored modules. You may override ",
      "this by providing the `--import-map <other-import-map>` flag or by ",
      "manually editing your Deno configuration file.",
    ),
    if dir != "vendor/" {
      format!(
        "{}{}",
        dir.trim_end_matches('/'),
        if cfg!(windows) { '\\' } else { '/' }
      )
    } else {
      dir.to_string()
    }
  )
}

fn ignoring_import_map_text() -> String {
  format!(
    concat!(
      "Ignoring import map. Specifying an import map file ({}) in the deno ",
      "vendor output directory is not supported. If you wish to use an ",
      "import map while vendoring, please specify one located outside this ",
      "directory.",
    ),
    PathBuf::from("vendor").join("import_map.json").display(),
  )
}
