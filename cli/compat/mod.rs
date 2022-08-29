// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::file_fetcher::FileFetcher;
use deno_ast::MediaType;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;

/// Translates given CJS module into ESM. This function will perform static
/// analysis on the file to find defined exports and reexports.
///
/// For all discovered reexports the analysis will be performed recursively.
///
/// If successful a source code for equivalent ES module is returned.
pub fn translate_cjs_to_esm(
  file_fetcher: &FileFetcher,
  specifier: &ModuleSpecifier,
  code: String,
  media_type: MediaType,
) -> Result<String, AnyError> {
  let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
    specifier: specifier.to_string(),
    text_info: deno_ast::SourceTextInfo::new(code.into()),
    media_type,
    capture_tokens: true,
    scope_analysis: false,
    maybe_syntax: None,
  })?;
  let analysis = parsed_source.analyze_cjs();

  let mut source = vec![
    r#"import { createRequire } from "node:module";"#.to_string(),
    r#"const require = createRequire(import.meta.url);"#.to_string(),
  ];

  // if there are reexports, handle them first
  for (idx, reexport) in analysis.reexports.iter().enumerate() {
    // Firstly, resolve relate reexport specifier
    let resolved_reexport = node_resolver::resolve(
      reexport,
      &specifier.to_file_path().unwrap(),
      // FIXME(bartlomieju): check if these conditions are okay, probably
      // should be `deno-require`, because `deno` is already used in `esm_resolver.rs`
      &["deno", "require", "default"],
    )?;
    let reexport_specifier =
      ModuleSpecifier::from_file_path(&resolved_reexport).unwrap();
    // Secondly, read the source code from disk
    let reexport_file = file_fetcher.get_source(&reexport_specifier).unwrap();
    // Now perform analysis again
    {
      let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
        specifier: reexport_specifier.to_string(),
        text_info: deno_ast::SourceTextInfo::new(reexport_file.source),
        media_type: reexport_file.media_type,
        capture_tokens: true,
        scope_analysis: false,
        maybe_syntax: None,
      })?;
      let analysis = parsed_source.analyze_cjs();

      source.push(format!(
        "const reexport{} = require(\"{}\");",
        idx, reexport
      ));

      for export in analysis.exports.iter().filter(|e| e.as_str() != "default")
      {
        // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
        // but it might not be necessary here since our analysis is more detailed?
        source.push(format!(
          "export const {0} = Deno[Deno.internal].require.bindExport(reexport{1}.{2}, reexport{1});",
          export, idx, export
        ));
      }
    }
  }

  source.push(format!(
    "const mod = require(\"{}\");",
    specifier
      .to_file_path()
      .unwrap()
      .to_str()
      .unwrap()
      .replace('\\', "\\\\")
      .replace('\'', "\\\'")
      .replace('\"', "\\\"")
  ));
  source.push("export default mod;".to_string());

  for export in analysis.exports.iter().filter(|e| e.as_str() != "default") {
    // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
    // but it might not be necessary here since our analysis is more detailed?
    source.push(format!(
      "export const {} = Deno[Deno.internal].require.bindExport(mod.{}, mod);",
      export, export
    ));
  }

  let translated_source = source.join("\n");
  Ok(translated_source)
}
