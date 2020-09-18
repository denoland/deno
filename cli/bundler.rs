use crate::module_graph::ModuleGraphFile;
use anyhow::bail;
use deno_core::ModuleSpecifier;
use std::{collections::HashMap, rc::Rc};
use swc_common::input::StringInput;
use swc_common::FileName;
use swc_common::FilePathMapping;
use swc_ecmascript::visit::FoldWith;
use swc_ecmascript::{
  ast::Module,
  codegen::text_writer::JsWriter,
  parser::{lexer::Lexer, JscTarget, Parser, Syntax},
};

pub fn bundle(
  module_graph: &HashMap<String, ModuleGraphFile>,
  entry: &ModuleSpecifier,
) -> Result<String, anyhow::Error> {
  let cm = Rc::new(swc_common::SourceMap::new(FilePathMapping::empty()));
  let loader = SwcLoader {
    cm: cm.clone(),
    module_graph: &module_graph,
  };
  let resolver = SwcResolver {
    module_graph: &module_graph,
  };
  let globals = swc_common::Globals::new();
  let bundler = swc_bundler::Bundler::new(
    &globals,
    cm.clone(),
    loader,
    resolver,
    swc_bundler::Config {
      require: false,
      // TODO(kdy1): Change this to false
      disable_inliner: true,
      external_modules: vec![],
    },
  );

  let mut entries = HashMap::default();
  entries.insert("bundle".to_string(), FileName::Custom(entry.to_string()));
  // TODO(kdy1): Remove expect
  let output = bundler.bundle(entries).expect("failed to bundle");
  let mut buf = vec![];
  {
    let mut emitter = swc_ecmascript::codegen::Emitter {
      cfg: swc_ecmascript::codegen::Config { minify: false },
      cm: cm.clone(),
      comments: None,
      wr: Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None)),
    };

    // Cannnot happen
    emitter.emit_module(&output[0].module).unwrap();
  }

  Ok(String::from_utf8(buf).expect("codegen should generate utf-8 output"))
}

struct SwcLoader<'a> {
  cm: Rc<swc_common::SourceMap>,
  module_graph: &'a HashMap<String, ModuleGraphFile>,
}

impl swc_bundler::Load for SwcLoader<'_> {
  fn load(
    &self,
    file: &FileName,
  ) -> Result<(Rc<swc_common::SourceFile>, Module), anyhow::Error> {
    match file {
      FileName::Custom(name) => {
        // We use custom
        if let Some(module) = self.module_graph.get(name) {
          let fm = self.cm.new_source_file(
            FileName::Custom(name.into()),
            module.source_code.clone(),
          );

          let lexer = Lexer::new(
            Syntax::Typescript(Default::default()),
            JscTarget::Es2020,
            StringInput::from(&*fm),
            None,
          );
          let mut p = Parser::new_from(lexer);

          let result = p.parse_module();
          let module = match result {
            Ok(v) => v,
            Err(err) => {
              bail!("Parsing failed: {:?}", err);
            }
          };

          let module = module
            .fold_with(&mut swc_ecmascript::transforms::typescript::strip());

          Ok((fm, module))
        } else {
          bail!("swc_bundler requested non-existant file {:?}", file)
        }
      }
      _ => unreachable!(
        "swc_bundler requested parsing of non-string named file {:?}",
        file
      ),
    }
  }
}

struct SwcResolver<'a> {
  module_graph: &'a HashMap<String, ModuleGraphFile>,
}

impl swc_bundler::Resolve for SwcResolver<'_> {
  fn resolve(
    &self,
    base: &FileName,
    module_specifier: &str,
  ) -> Result<FileName, anyhow::Error> {
    let base = if let FileName::Custom(base) = base {
      base
    } else {
      unreachable!()
    };

    if let Some(module) = self.module_graph.get(base) {
      for import in &module.imports {
        if import.specifier == module_specifier {
          return Ok(FileName::Custom(import.resolved_specifier.to_string()));
        }
      }
    }

    dbg!(base, module_specifier);
    unimplemented!()
  }
}
