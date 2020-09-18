use crate::{
  global_state::GlobalState,
  module_graph::{ModuleGraphFile, ModuleGraphLoader},
  permissions::Permissions,
};
use anyhow::{bail, Context};
use deno_core::{error::AnyError, ModuleSpecifier};
use std::{collections::HashMap, rc::Rc, sync::Arc};
use swc_common::input::StringInput;
use swc_common::FileName;
use swc_common::FilePathMapping;
use swc_ecmascript::visit::FoldWith;
use swc_ecmascript::{
  ast::Module,
  codegen::text_writer::JsWriter,
  parser::{lexer::Lexer, JscTarget, Parser, Syntax},
};

/// For a given module, generate a single file JavaScript output that includes
/// all the dependencies for that module.
pub async fn bundle(
  global_state: &Arc<GlobalState>,
  module_specifier: ModuleSpecifier,
) -> Result<String, AnyError> {
  let permissions = Permissions::allow_all();
  let mut module_graph_loader = ModuleGraphLoader::new(
    global_state.ts_compiler.file_fetcher.clone(),
    global_state.maybe_import_map.clone(),
    permissions.clone(),
    false,
    true,
  );
  module_graph_loader
    .add_to_graph(&module_specifier, None)
    .await?;
  let module_graph = module_graph_loader.get_graph();

  bundle_graph(&module_graph, &module_specifier)
}

pub fn bundle_graph(
  module_graph: &HashMap<String, ModuleGraphFile>,
  entry: &ModuleSpecifier,
) -> Result<String, AnyError> {
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
      disable_inliner: false,
      external_modules: vec![],
    },
  );

  let mut entries = HashMap::default();
  entries.insert("bundle".to_string(), FileName::Custom(entry.to_string()));
  let output = bundler.bundle(entries).context("failed to bundle")?;
  let mut buf = vec![];
  {
    let mut emitter = swc_ecmascript::codegen::Emitter {
      cfg: swc_ecmascript::codegen::Config { minify: false },
      cm: cm.clone(),
      comments: None,
      wr: Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None)),
    };

    // Cannnot happen
    emitter
      .emit_module(&output[0].module)
      .context("failed to emit module")?;
  }

  let s = String::from_utf8(buf).context("swc emitted non-utf8 string")?;
  Ok(s)
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
      unreachable!("swc_bundler gave non-string filename: {:?}", base)
    };

    if let Some(module) = self.module_graph.get(base) {
      for import in &module.imports {
        if import.specifier == module_specifier {
          return Ok(FileName::Custom(import.resolved_specifier.to_string()));
        }
      }
    }

    bail!("failed to resolve {} from {}", module_specifier, base)
  }
}
