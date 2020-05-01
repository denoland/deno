// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![allow(unused)]

use crate::file_fetcher::SourceFileFetcher;
use crate::swc_util::analyze_dependencies;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
struct ModuleGraph(HashMap<String, ModuleGraphFile>);

#[derive(Debug, Serialize)]
struct ModuleGraphFile {
  pub specifier: String,
  pub deps: Vec<String>,
}

struct ModuleGraphLoader {
  file_fetcher: SourceFileFetcher,
  to_visit: Vec<ModuleSpecifier>,
  pub graph: ModuleGraph,
}

// struct ModuleGraphFuture {
//   file_fetcher: SourceFileFetcher,
//   to_visit: Vec<ModuleSpecifier>,
//   pending_lods:
//     FuturesUnordered<Pin<Box<dyn Future<Output = Result<SourceFile, ErrBox>>>>>,
//   has_loaded: HashSet<ModuleSpecifier>,
//   pending_analysis: HashSet<ModuleSpecifier>,
// }

impl ModuleGraphLoader {
  pub fn new(file_fetcher: SourceFileFetcher) -> Self {
    Self {
      file_fetcher,
      to_visit: vec![],
      graph: ModuleGraph(HashMap::new()),
    }
  }

  pub async fn build_graph(
    mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<HashMap<String, ModuleGraphFile>, ErrBox> {
    self.to_visit.push(specifier.to_owned());
    while let Some(spec) = self.to_visit.pop() {
      self.visit_module(&spec).await?;
      let file = self.graph.0.get(&spec.to_string()).unwrap();
      for dep in &file.deps {
        self
          .to_visit
          .push(ModuleSpecifier::resolve_url_or_path(dep).unwrap());
      }
    }
    Ok(self.graph.0)
  }

  async fn visit_module(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<(), ErrBox> {
    if self.graph.0.contains_key(&specifier.to_string()) {
      return Ok(());
    }

    let source_file =
      self.file_fetcher.fetch_source_file(specifier, None).await?;

    let raw_deps =
      analyze_dependencies(&String::from_utf8(source_file.source_code)?, true)?;

    // TODO(bartlomieju): apply import map, using State
    //    or should it be passed explicitly
    let mut deps = vec![];
    for raw_dep in raw_deps {
      let specifier =
        ModuleSpecifier::resolve_import(&raw_dep, &specifier.to_string())?;
      deps.push(specifier.to_string());
    }

    self.graph.0.insert(
      specifier.to_string(),
      ModuleGraphFile {
        specifier: specifier.to_string(),
        deps,
      },
    );
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::GlobalState;
  use std::path::PathBuf;

  fn rel_module_specifier(relpath: &str) -> ModuleSpecifier {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join(relpath)
      .into_os_string();
    let ps = p.to_str().unwrap();
    // TODO(ry) Why doesn't ModuleSpecifier::resolve_path actually take a
    // Path?!
    ModuleSpecifier::resolve_url_or_path(ps).unwrap()
  }

  #[tokio::test]
  async fn source_graph_fetch() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = rel_module_specifier("tests/019_media_types.ts");

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone());
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    assert_eq!(graph.len(), 9);
    let r = graph
      .get("http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts");
    assert!(r.is_some());

    println!("{}", serde_json::to_string_pretty(&graph).unwrap());

    drop(http_server_guard);
  }
}
