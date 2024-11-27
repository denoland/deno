use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use deno_core::{anyhow::Context, error::AnyError, url::Url};
use deno_graph::{GraphKind, Module, ModuleGraph, NpmModule};
use deno_runtime::deno_node::NodeResolver;
use indexmap::IndexSet;
use node_resolver::{NodeModuleKind, NodeResolutionMode};

use crate::{
  graph_util::{CreateGraphOptions, ModuleGraphCreator},
  npm::CliNpmResolver,
  tools::bundle::bundle_graph::BundleDep,
};

use super::bundle_graph::{BundleGraph, BundleJsModule, BundleModule};

pub async fn build_resolved_graph(
  module_graph_creator: &Arc<ModuleGraphCreator>,
  npm_resolver: &Arc<dyn CliNpmResolver>,
  node_resolver: &Arc<NodeResolver>,
  files: Vec<Url>,
) -> Result<BundleGraph, AnyError> {
  let graph = module_graph_creator
    .create_graph_with_options(CreateGraphOptions {
      graph_kind: GraphKind::CodeOnly,
      roots: files.clone(),
      is_dynamic: false,
      loader: None,
    })
    .await?;

  graph.valid()?;

  let mut bundle_graph = BundleGraph::new();

  let mut npm_modules: IndexSet<Url> = IndexSet::new();
  let mut seen: HashSet<Url> = HashSet::new();
  let mut pending_npm_dep_links: HashMap<usize, Url> = HashMap::new();

  walk_graph(
    &graph,
    &mut seen,
    npm_resolver,
    node_resolver,
    &mut bundle_graph,
    &mut npm_modules,
    &mut pending_npm_dep_links,
  );

  // Resolve npm modules
  // Hack: Create sub graphs for every npm module we encounter and
  // expand npm specifiers to the actual file
  while let Some(url) = npm_modules.pop() {
    let npm_graph = module_graph_creator
      .create_graph_with_options(CreateGraphOptions {
        graph_kind: GraphKind::CodeOnly,
        roots: vec![url.clone()],
        is_dynamic: false,
        loader: None,
      })
      .await?;

    walk_graph(
      &npm_graph,
      &mut seen,
      npm_resolver,
      node_resolver,
      &mut bundle_graph,
      &mut npm_modules,
      &mut pending_npm_dep_links,
    );
  }

  Ok(bundle_graph)
}

fn walk_graph(
  graph: &ModuleGraph,
  seen: &mut HashSet<Url>,
  npm_resolver: &Arc<dyn CliNpmResolver>,
  node_resolver: &Arc<NodeResolver>,
  bundle_graph: &mut BundleGraph,
  npm_modules: &mut IndexSet<Url>,
  pending_npm_dep_links: &mut HashMap<usize, Url>,
) {
  for module in graph.modules() {
    let url = module.specifier();

    if seen.contains(&url) {
      continue;
    }

    seen.insert(url.clone());

    match module {
      Module::Npm(module) => {
        let resolved = resolve_npm_module(&module, npm_resolver, node_resolver);
        npm_modules.insert(resolved.clone());
      }
      Module::Js(js_module) => {
        let id = bundle_graph.insert(
          url.clone(),
          BundleModule::Js(BundleJsModule {
            specifier: url.clone(),
            media_type: js_module.media_type,
            source: js_module.source.to_string(),
            ast: None,
            dependencies: vec![],
          }),
        );

        for (raw, dep) in &js_module.dependencies {
          if let Some(code) = dep.get_code() {
            if code.scheme() == "npm" {
              pending_npm_dep_links.insert(id, code.clone());
            } else {
              let dep_id = bundle_graph.register(code.clone());
              bundle_graph.add_dependency(
                id,
                BundleDep {
                  id: dep_id,
                  raw: raw.to_string(),
                  is_dyanmic: dep.is_dynamic,
                },
              );
            }
          }
        }
      }
      Module::Json(json_module) => {
        bundle_graph
          .insert(url.clone(), BundleModule::Json(json_module.clone()));
      }
      Module::Wasm(wasm_module) => {
        bundle_graph
          .insert(url.clone(), BundleModule::Wasm(wasm_module.clone()));
      }
      Module::Node(built_in_node_module) => {
        bundle_graph.insert(
          url.clone(),
          BundleModule::Node(built_in_node_module.module_name.to_string()),
        );
      }
      Module::External(external_module) => todo!(),
    }
  }
}

fn resolve_npm_module(
  module: &NpmModule,
  npm_resolver: &Arc<dyn CliNpmResolver>,
  node_resolver: &Arc<NodeResolver>,
) -> Url {
  let nv = module.nv_reference.nv();
  let managed = npm_resolver.as_managed().unwrap();
  let package_folder = managed.resolve_pkg_folder_from_deno_module(nv).unwrap();

  let resolved = node_resolver
    .resolve_package_subpath_from_deno_module(
      &package_folder,
      module.nv_reference.sub_path(),
      None,                // FIXME
      NodeModuleKind::Esm, // FIXME
      NodeResolutionMode::Execution,
    )
    .with_context(|| format!("Could not resolve '{}'.", module.nv_reference))
    .unwrap();

  resolved
}
