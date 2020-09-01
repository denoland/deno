#![allow(unused)]

use anyhow::Error;
use deno_core::ModuleSpecifier;
use std::path::PathBuf;
use std::{collections::HashMap, io::stdout};
use swc_bundler::{BundleKind, Bundler, Config, Load, Resolve};
use swc_common::{sync::Lrc, FileName, FilePathMapping, Globals, SourceMap};
use swc_ecmascript::ast;
use swc_ecmascript::codegen::{
  text_writer::JsWriter, Config as CodegenConfig, Emitter,
};
use swc_ecmascript::parser::{
  lexer::Lexer, EsConfig, Parser, StringInput, Syntax,
};

// TODO(bartlomieju): figure out how it would work
// with TS sources - problematic part might be 
// "double source map" - first one from compiler TS to JS
// and the second for actual bundle
// Probably the bundler can figure it out on its own
// if source map is inlined in the file itself.
fn bundle() -> Result<String, Error> {
  let globals = Globals::new();
  let cm = Lrc::new(SourceMap::new(FilePathMapping::empty()));
  // This example does not use core modules.
  let external_modules = vec![];
  let bundler = Bundler::new(
    &globals,
    cm.clone(),
    PathLoader { cm: cm.clone() },
    PathResolver,
    Config {
      require: false,
      external_modules,
    },
  );
  let mut entries = HashMap::default();
  entries.insert(
    "bundle_main".to_string(),
    FileName::Real("./bundle_entry.js".into()),
  );

  // TODO(bartlomieju): figure out how it works with statically 
  // determinable dynamic imports (import("./foo.ts"))
  let mut bundles = bundler.bundle(entries).expect("failed to bundle");
  assert_eq!(
    bundles.len(),
    1,
    "There's no conditional / dynamic imports and we provided only one entry"
  );
  let bundle = bundles.pop().unwrap();
  assert_eq!(
    bundle.kind,
    BundleKind::Named {
      name: "bundle_main".into()
    },
    "We provided it"
  );

  let mut buf = vec![];

  {
    let writer = Box::new(JsWriter::new(
      cm.clone(),
      "\n",
      &mut buf,
      // TODO: source map handling
      None,
    ));
    let mut emitter = Emitter {
      cfg: CodegenConfig { minify: false },
      cm: cm.clone(),
      comments: None,
      wr: writer,
    };

    emitter.emit_module(&bundle.module)?;
  }

  let source = String::from_utf8(buf)?;
  Ok(source)
}

struct PathLoader {
  cm: Lrc<SourceMap>,
}

// TODO(bartlomieju): this struct should:
// - use SourceFileFetcher
// - use MediaType to decide on the synta
// - use AstParser from swc_util.rs to perform the parsing
impl Load for PathLoader {
  fn load(
    &self,
    file: &FileName,
  ) -> Result<(Lrc<swc_common::SourceFile>, ast::Module), Error> {
    let filename = match file {
      FileName::Real(v) => v,
      _ => unreachable!(),
    };

    eprintln!("filename {:#?}", filename);
    let source_text = std::fs::read_to_string(filename)?;

    let fm = self.cm.new_source_file(file.clone(), source_text);
    // TODO(bartlomieju): configure based on the MediaType
    let lexer = Lexer::new(
      Syntax::Es(EsConfig {
        ..Default::default()
      }),
      Default::default(),
      StringInput::from(&*fm),
      None,
    );

    let mut parser = Parser::new_from(lexer);
    let module = parser.parse_module().expect("This should not happen");

    Ok((fm, module))
  }
}

// TODO(bartlomieju): use ModuleSpecifier to resolve the specifier
// and transform them back to filenames
struct PathResolver;

impl Resolve for PathResolver {
  fn resolve(
    &self,
    referrer: &FileName,
    specifier: &str,
  ) -> Result<FileName, Error> {
    let referrer_str = match referrer {
      FileName::Real(v) => v,
      _ => unreachable!(),
    };

    // TODO(bartlomieju): handle errors
    // let module_specifier = ModuleSpecifier::resolve_import(specifier, &referrer_str).expect("Invalid module specifiers");
    let path = referrer_str.parent().unwrap().join(specifier);

    Ok(FileName::Real(path))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_bundler() {
    let result = bundle();
    assert!(result.is_ok());
    eprintln!("bundled source {:#?}", result.unwrap());
  }
}
