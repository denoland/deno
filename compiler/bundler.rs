// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::source_map_bundler::SourceMapBundler;
use crate::Result;

use deno_core::ErrBox;
use regex::Regex;
use std::error::Error;
use std::fmt;

static SYSTEM_LOADER_CODE: &str = include_str!("system_loader.js");
static SYSTEM_LOADER_ES5_CODE: &str = include_str!("system_loader_es5.js");

lazy_static! {
  static ref SOURCE_MAPPING_URL_RE: Regex =
    Regex::new(r#"(?m)^//#\ssourceMappingURL=.+$"#).unwrap();
}

fn count_newlines(s: &str) -> usize {
  bytecount::count(s.as_bytes(), b'\n')
}

struct BundleError(String);

impl fmt::Display for BundleError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Bundle Error: {}", self.0)
  }
}

impl fmt::Debug for BundleError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "BundleError {{ message: {} }}", self.0)
  }
}

impl Error for BundleError {}

pub struct BundleFile {
  pub code: String,
  pub map: String,
}

#[derive(Default)]
pub struct BundleOptions {
  /// If `true` the source map will be inlined into the bundled JavaScript file.
  /// Otherwise it will be returned in the result.
  pub inline_source_map: bool,
  /// The main module specifier to initialize to bootstrap the bundle.
  pub main_specifier: String,
  /// Any named exports that the main module specifier might have that should be
  /// re-exported in the bundle.
  pub maybe_named_exports: Option<Vec<String>>,
  /// If `false` then a bundle loader that is compatible with ES2017 or later
  /// will be used to bootstrap the module. If `true` then a bundle loader that
  /// is compatible with ES5 or later will be used.  *Note* when targeting ES5
  /// the `bundle()` function will error if the bundle requires top level await
  /// for one of the modules.
  pub target_es5: bool,
}

/// Take a vector of files and a structure of options and output a single file
/// JavaScript file bundle.
///
/// # Arguments
///
/// * `files` - A vector of `BundleFile`s, which are preprocessed files where
///   the code is a SystemJS module with a module specifier and the source map
///   is a map between the original source code and the supplied code.
/// * `options` - A structure of options which affect the behavior of the
///   function.
///
/// # Errors
///
/// The function will error if there are issues processing the source map files
/// or if there are incompatible option settings.
///
pub fn bundle(
  files: Vec<BundleFile>,
  options: BundleOptions,
) -> Result<(String, Option<String>)> {
  let mut code = String::new();
  let preamble = if options.target_es5 {
    SYSTEM_LOADER_ES5_CODE
  } else {
    SYSTEM_LOADER_CODE
  };
  code.push_str(preamble);
  let mut source_map_bundle = SourceMapBundler::new(None);
  for file in files.iter() {
    let line_offset = count_newlines(&code);
    let file_code = SOURCE_MAPPING_URL_RE.replace(&file.code, "");
    code.push_str(&file_code);
    source_map_bundle.append_from_str(&file.map, line_offset)?;
  }
  let has_exports = options.maybe_named_exports.is_some();
  let top_level_await = code.contains("execute: async function");
  if top_level_await && options.target_es5 {
    let message = "The bundle target does not support top level await, but top level await is required.".to_string();
    return Err(ErrBox::from(BundleError(message)));
  }
  let init_code = if has_exports {
    if top_level_await {
      format!(
        "\nvar __exp = await __instantiate(\"{}\", true);\n",
        options.main_specifier
      )
    } else {
      format!(
        "\nvar __exp = __instantiate(\"{}\", false);\n",
        options.main_specifier
      )
    }
  } else if top_level_await {
    format!(
      "\nawait __instantiate(\"{}\", true);\n",
      options.main_specifier
    )
  } else {
    format!("\n__instantiate(\"{}\", false);\n", options.main_specifier)
  };
  code.push_str(&init_code);
  if let Some(named_exports) = options.maybe_named_exports {
    for named_export in named_exports.iter() {
      let export_code = match named_export.as_str() {
        "default" => "export default __exp[\"default\"];\n".to_string(),
        _ => format!(
          "export var {} = __exp[\"{}\"];\n",
          named_export, named_export
        ),
      };
      code.push_str(&export_code);
    }
  };
  let mut map_bytes: Vec<u8> = vec![];
  source_map_bundle
    .into_sourcemap()
    .to_writer(&mut map_bytes)?;

  if options.inline_source_map {
    let map_base64 = base64::encode(map_bytes);
    let map_pragma = format!(
      "\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,{}\n",
      map_base64
    );
    code.push_str(&map_pragma);

    Ok((code, None))
  } else {
    let maybe_map = Some(String::from_utf8(map_bytes)?);

    Ok((code, maybe_map))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_bundle() {
    let (code, maybe_map) = bundle(
      vec![BundleFile {
        code: r#"System.register("https://deno.land/x/a.ts", [], function (exports_1, context_1) {
    "use strict";
    var __moduleName = context_1 && context_1.id;
    return {
        setters: [],
        execute: function () {
            console.log("hello deno");
        }
    };
});
"#.to_string(),
        map: r#"{
  "version": 3,
  "sources": ["coolstuff.js"],
  "names": ["x", "alert"],
  "mappings": "AAAA,GAAIA,GAAI,EACR,IAAIA,GAAK,EAAG,CACVC,MAAM"
}"#.to_string(),
      }],
      BundleOptions {
        inline_source_map: true,
        main_specifier: "https://deno.land/x/a.ts".to_string(),
        maybe_named_exports: None,
        target_es5: false,
      },
    )
    .unwrap();
    println!("{}", code);
    assert!(code.starts_with("// Copyright 2018"));
    assert!(maybe_map.is_none());
  }
}
