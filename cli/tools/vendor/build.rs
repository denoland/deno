// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use deno_core::error::AnyError;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::ModuleKind;

use super::analyze::has_default_export;
use super::import_map::build_import_map;
use super::mappings::Mappings;
use super::mappings::ProxiedModule;
use super::specifiers::is_remote_specifier;

/// Allows substituting the environment for testing purposes.
pub trait VendorEnvironment {
  fn create_dir_all(&self, dir_path: &Path) -> Result<(), AnyError>;
  fn write_file(&self, file_path: &Path, text: &str) -> Result<(), AnyError>;
}

pub struct RealVendorEnvironment;

impl VendorEnvironment for RealVendorEnvironment {
  fn create_dir_all(&self, dir_path: &Path) -> Result<(), AnyError> {
    Ok(std::fs::create_dir_all(dir_path)?)
  }

  fn write_file(&self, file_path: &Path, text: &str) -> Result<(), AnyError> {
    Ok(std::fs::write(file_path, text)?)
  }
}

/// Vendors remote modules and returns how many were vendored.
pub fn build(
  graph: &ModuleGraph,
  output_dir: &Path,
  environment: &impl VendorEnvironment,
) -> Result<usize, AnyError> {
  assert!(output_dir.is_absolute());
  let all_modules = graph.modules();
  let remote_modules = all_modules
    .iter()
    .filter(|m| is_remote_specifier(&m.specifier))
    .copied()
    .collect::<Vec<_>>();
  let mappings =
    Mappings::from_remote_modules(graph, &remote_modules, output_dir)?;

  // write out all the files
  for module in &remote_modules {
    let source = match &module.maybe_source {
      Some(source) => source,
      None => continue,
    };
    let local_path = mappings
      .proxied_path(&module.specifier)
      .unwrap_or_else(|| mappings.local_path(&module.specifier));
    if !matches!(module.kind, ModuleKind::Esm | ModuleKind::Asserted) {
      log::warn!(
        "Unsupported module kind {:?} for {}",
        module.kind,
        module.specifier
      );
      continue;
    }
    environment.create_dir_all(local_path.parent().unwrap())?;
    environment.write_file(&local_path, source)?;
  }

  // write out the proxies
  for (specifier, proxied_module) in mappings.proxied_modules() {
    let proxy_path = mappings.local_path(specifier);
    let module = graph.get(specifier).unwrap();
    let text = build_proxy_module_source(module, proxied_module);

    environment.write_file(&proxy_path, &text)?;
  }

  // create the import map
  if !mappings.base_specifiers().is_empty() {
    let import_map_text = build_import_map(graph, &all_modules, &mappings);
    environment
      .write_file(&output_dir.join("import_map.json"), &import_map_text)?;
  }

  Ok(remote_modules.len())
}

fn build_proxy_module_source(
  module: &Module,
  proxied_module: &ProxiedModule,
) -> String {
  let mut text = format!(
    "// @deno-types=\"{}\"\n",
    proxied_module.declaration_specifier
  );
  let relative_specifier = format!(
    "./{}",
    proxied_module
      .output_path
      .file_name()
      .unwrap()
      .to_string_lossy()
  );

  // for simplicity, always include the `export *` statement as it won't error
  // even when the module does not contain a named export
  text.push_str(&format!("export * from \"{}\";\n", relative_specifier));

  // add a default export if one exists in the module
  if let Some(parsed_source) = module.maybe_parsed_source.as_ref() {
    if has_default_export(parsed_source) {
      text.push_str(&format!(
        "export {{ default }} from \"{}\";\n",
        relative_specifier
      ));
    }
  }

  text
}

#[cfg(test)]
mod test {
  use crate::tools::vendor::test::VendorTestBuilder;
  use deno_core::serde_json::json;
  use pretty_assertions::assert_eq;

  #[tokio::test]
  async fn no_remote_modules() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader.add("/mod.ts", "");
      })
      .build()
      .await
      .unwrap();

    assert_eq!(output.import_map, None,);
    assert_eq!(output.files, vec![],);
  }

  #[tokio::test]
  async fn local_specifiers_to_remote() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add(
            "/mod.ts",
            concat!(
              r#"import "https://localhost/mod.ts";"#,
              r#"import "https://localhost/other.ts?test";"#,
              r#"import "https://localhost/redirect.ts";"#,
            ),
          )
          .add("https://localhost/mod.ts", "export class Mod {}")
          .add("https://localhost/other.ts?test", "export class Other {}")
          .add_redirect(
            "https://localhost/redirect.ts",
            "https://localhost/mod.ts",
          );
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/",
          "https://localhost/other.ts?test": "./localhost/other.ts",
          "https://localhost/redirect.ts": "./localhost/mod.ts",
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        ("/vendor/localhost/mod.ts", "export class Mod {}"),
        ("/vendor/localhost/other.ts", "export class Other {}"),
      ]),
    );
  }

  #[tokio::test]
  async fn remote_specifiers() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add(
            "/mod.ts",
            concat!(
              r#"import "https://localhost/mod.ts";"#,
              r#"import "https://other/mod.ts";"#,
            ),
          )
          .add(
            "https://localhost/mod.ts",
            concat!(
              "export * from './other.ts';",
              "export * from './redirect.ts';",
              "export * from '/absolute.ts';",
            ),
          )
          .add("https://localhost/other.ts", "export class Other {}")
          .add_redirect(
            "https://localhost/redirect.ts",
            "https://localhost/other.ts",
          )
          .add("https://localhost/absolute.ts", "export class Absolute {}")
          .add("https://other/mod.ts", "export * from './sub/mod.ts';")
          .add(
            "https://other/sub/mod.ts",
            concat!(
              "export * from '../sub2/mod.ts';",
              "export * from '../sub2/other?asdf';",
              // reference a path on a different origin
              "export * from 'https://localhost/other.ts';",
              "export * from 'https://localhost/redirect.ts';",
            ),
          )
          .add("https://other/sub2/mod.ts", "export class Mod {}")
          .add_with_headers(
            "https://other/sub2/other?asdf",
            "export class Other {}",
            &[("content-type", "application/javascript")],
          );
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/",
          "https://localhost/redirect.ts": "./localhost/other.ts",
          "https://other/": "./other/"
        },
        "scopes": {
          "./localhost/": {
            "./localhost/redirect.ts": "./localhost/other.ts",
            "/absolute.ts": "./localhost/absolute.ts",
          },
          "./other/": {
            "./other/sub2/other?asdf": "./other/sub2/other.js"
          }
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        ("/vendor/localhost/absolute.ts", "export class Absolute {}"),
        (
          "/vendor/localhost/mod.ts",
          concat!(
            "export * from './other.ts';",
            "export * from './redirect.ts';",
            "export * from '/absolute.ts';",
          )
        ),
        ("/vendor/localhost/other.ts", "export class Other {}"),
        ("/vendor/other/mod.ts", "export * from './sub/mod.ts';"),
        (
          "/vendor/other/sub/mod.ts",
          concat!(
            "export * from '../sub2/mod.ts';",
            "export * from '../sub2/other?asdf';",
            "export * from 'https://localhost/other.ts';",
            "export * from 'https://localhost/redirect.ts';",
          )
        ),
        ("/vendor/other/sub2/mod.ts", "export class Mod {}"),
        ("/vendor/other/sub2/other.js", "export class Other {}"),
      ]),
    );
  }

  #[tokio::test]
  async fn same_target_filename_specifiers() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add(
            "/mod.ts",
            concat!(
              r#"import "https://localhost/MOD.TS";"#,
              r#"import "https://localhost/mod.TS";"#,
              r#"import "https://localhost/mod.ts";"#,
              r#"import "https://localhost/mod.ts?test";"#,
              r#"import "https://localhost/CAPS.TS";"#,
            ),
          )
          .add("https://localhost/MOD.TS", "export class Mod {}")
          .add("https://localhost/mod.TS", "export class Mod2 {}")
          .add("https://localhost/mod.ts", "export class Mod3 {}")
          .add("https://localhost/mod.ts?test", "export class Mod4 {}")
          .add("https://localhost/CAPS.TS", "export class Caps {}");
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/",
          "https://localhost/mod.TS": "./localhost/mod_2.TS",
          "https://localhost/mod.ts": "./localhost/mod_3.ts",
          "https://localhost/mod.ts?test": "./localhost/mod_4.ts",
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        ("/vendor/localhost/CAPS.TS", "export class Caps {}"),
        ("/vendor/localhost/MOD.TS", "export class Mod {}"),
        ("/vendor/localhost/mod_2.TS", "export class Mod2 {}"),
        ("/vendor/localhost/mod_3.ts", "export class Mod3 {}"),
        ("/vendor/localhost/mod_4.ts", "export class Mod4 {}"),
      ]),
    );
  }

  #[tokio::test]
  async fn multiple_entrypoints() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .add_entry_point("/test.deps.ts")
      .with_loader(|loader| {
        loader
          .add("/mod.ts", r#"import "https://localhost/mod.ts";"#)
          .add(
            "/test.deps.ts",
            r#"export * from "https://localhost/test.ts";"#,
          )
          .add("https://localhost/mod.ts", "export class Mod {}")
          .add("https://localhost/test.ts", "export class Test {}");
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/",
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        ("/vendor/localhost/mod.ts", "export class Mod {}"),
        ("/vendor/localhost/test.ts", "export class Test {}"),
      ]),
    );
  }

  #[tokio::test]
  async fn json_module() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add(
            "/mod.ts",
            r#"import data from "https://localhost/data.json" assert { type: "json" };"#,
          )
          .add("https://localhost/data.json", "{}");
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/"
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[("/vendor/localhost/data.json", "{}"),]),
    );
  }

  #[tokio::test]
  async fn data_urls() {
    let mut builder = VendorTestBuilder::with_default_setup();

    let mod_file_text = r#"import * as b from "data:application/typescript,export%20*%20from%20%22https://localhost/mod.ts%22;";"#;

    let output = builder
      .with_loader(|loader| {
        loader
          .add("/mod.ts", &mod_file_text)
          .add("https://localhost/mod.ts", "export class Example {}");
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/"
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[("/vendor/localhost/mod.ts", "export class Example {}"),]),
    );
  }

  #[tokio::test]
  async fn x_typescript_types_no_default() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add("/mod.ts", r#"import "https://localhost/mod.js";"#)
          .add_with_headers(
            "https://localhost/mod.js",
            "export class Mod {}",
            &[("x-typescript-types", "https://localhost/mod.d.ts")],
          )
          .add("https://localhost/mod.d.ts", "export class Mod {}");
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/"
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        ("/vendor/localhost/mod.d.ts", "export class Mod {}"),
        (
          "/vendor/localhost/mod.js",
          concat!(
            "// @deno-types=\"https://localhost/mod.d.ts\"\n",
            "export * from \"./mod.proxied.js\";\n"
          )
        ),
        ("/vendor/localhost/mod.proxied.js", "export class Mod {}"),
      ]),
    );
  }

  #[tokio::test]
  async fn x_typescript_types_default_export() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add("/mod.ts", r#"import "https://localhost/mod.js";"#)
          .add_with_headers(
            "https://localhost/mod.js",
            "export default class Mod {}",
            &[("x-typescript-types", "https://localhost/mod.d.ts")],
          )
          .add("https://localhost/mod.d.ts", "export default class Mod {}");
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/"
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        ("/vendor/localhost/mod.d.ts", "export default class Mod {}"),
        (
          "/vendor/localhost/mod.js",
          concat!(
            "// @deno-types=\"https://localhost/mod.d.ts\"\n",
            "export * from \"./mod.proxied.js\";\n",
            "export { default } from \"./mod.proxied.js\";\n",
          )
        ),
        (
          "/vendor/localhost/mod.proxied.js",
          "export default class Mod {}"
        ),
      ]),
    );
  }

  #[tokio::test]
  async fn subdir() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add(
            "/mod.ts",
            r#"import "http://localhost:4545/sub/logger/mod.ts?testing";"#,
          )
          .add(
            "http://localhost:4545/sub/logger/mod.ts?testing",
            "export * from './logger.ts?test';",
          )
          .add(
            "http://localhost:4545/sub/logger/logger.ts?test",
            "export class Logger {}",
          );
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "http://localhost:4545/": "./localhost_4545/",
          "http://localhost:4545/sub/logger/mod.ts?testing": "./localhost_4545/sub/logger/mod.ts",
        },
        "scopes": {
          "./localhost_4545/": {
            "./localhost_4545/sub/logger/logger.ts?test": "./localhost_4545/sub/logger/logger.ts"
          }
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        (
          "/vendor/localhost_4545/sub/logger/logger.ts",
          "export class Logger {}",
        ),
        (
          "/vendor/localhost_4545/sub/logger/mod.ts",
          "export * from './logger.ts?test';"
        ),
      ]),
    );
  }

  #[tokio::test]
  async fn same_origin_absolute_with_redirect() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add(
            "/mod.ts",
            r#"import "https://localhost/subdir/sub/mod.ts";"#,
          )
          .add(
            "https://localhost/subdir/sub/mod.ts",
            "import 'https://localhost/std/hash/mod.ts'",
          )
          .add_redirect(
            "https://localhost/std/hash/mod.ts",
            "https://localhost/std@0.1.0/hash/mod.ts",
          )
          .add(
            "https://localhost/std@0.1.0/hash/mod.ts",
            "export class Test {}",
          );
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost/",
          "https://localhost/std/hash/mod.ts": "./localhost/std@0.1.0/hash/mod.ts"
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        (
          "/vendor/localhost/std@0.1.0/hash/mod.ts",
          "export class Test {}"
        ),
        (
          "/vendor/localhost/subdir/sub/mod.ts",
          "import 'https://localhost/std/hash/mod.ts'"
        ),
      ]),
    );
  }

  fn to_file_vec(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
      .iter()
      .map(|(f, t)| (f.to_string(), t.to_string()))
      .collect()
  }
}
