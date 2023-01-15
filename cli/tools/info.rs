// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Write;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_graph::Dependency;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleKind;
use deno_graph::Resolved;
use deno_runtime::colors;

use crate::args::Flags;
use crate::args::InfoFlags;
use crate::display;
use crate::npm::NpmPackageId;
use crate::npm::NpmPackageReference;
use crate::npm::NpmPackageReq;
use crate::npm::NpmPackageResolver;
use crate::npm::NpmResolutionPackage;
use crate::npm::NpmResolutionSnapshot;
use crate::proc_state::ProcState;
use crate::util::checksum;

pub async fn info(flags: Flags, info_flags: InfoFlags) -> Result<(), AnyError> {
  let ps = ProcState::build(flags).await?;
  if let Some(specifier) = info_flags.file {
    let specifier = resolve_url_or_path(&specifier)?;
    let graph = ps.create_graph(vec![(specifier, ModuleKind::Esm)]).await?;

    if info_flags.json {
      let mut json_graph = json!(graph);
      add_npm_packages_to_json(&mut json_graph, &ps.npm_resolver);
      display::write_json_to_stdout(&json_graph)?;
    } else {
      let mut output = String::new();
      GraphDisplayContext::write(&graph, &ps.npm_resolver, &mut output)?;
      display::write_to_stdout_ignore_sigpipe(output.as_bytes())?;
    }
  } else {
    // If it was just "deno info" print location of caches and exit
    print_cache_info(
      &ps,
      info_flags.json,
      ps.options.location_flag().as_ref(),
    )?;
  }
  Ok(())
}

fn print_cache_info(
  state: &ProcState,
  json: bool,
  location: Option<&deno_core::url::Url>,
) -> Result<(), AnyError> {
  let deno_dir = &state.dir.root_path_for_display();
  let modules_cache = &state.file_fetcher.get_http_cache_location();
  let npm_cache = &state.npm_cache.as_readonly().get_cache_location();
  let typescript_cache = &state.dir.gen_cache.location;
  let registry_cache = &state.dir.registries_folder_path();
  let mut origin_dir = state.dir.origin_data_folder_path();

  if let Some(location) = &location {
    origin_dir =
      origin_dir.join(checksum::gen(&[location.to_string().as_bytes()]));
  }

  let local_storage_dir = origin_dir.join("local_storage");

  if json {
    let mut output = json!({
      "denoDir": deno_dir.to_string(),
      "modulesCache": modules_cache,
      "npmCache": npm_cache,
      "typescriptCache": typescript_cache,
      "registryCache": registry_cache,
      "originStorage": origin_dir,
    });

    if location.is_some() {
      output["localStorage"] = serde_json::to_value(local_storage_dir)?;
    }

    display::write_json_to_stdout(&output)
  } else {
    println!("{} {}", colors::bold("DENO_DIR location:"), deno_dir);
    println!(
      "{} {}",
      colors::bold("Remote modules cache:"),
      modules_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("npm modules cache:"),
      npm_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("Emitted modules cache:"),
      typescript_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("Language server registries cache:"),
      registry_cache.display(),
    );
    println!(
      "{} {}",
      colors::bold("Origin storage:"),
      origin_dir.display()
    );
    if location.is_some() {
      println!(
        "{} {}",
        colors::bold("Local Storage:"),
        local_storage_dir.display(),
      );
    }
    Ok(())
  }
}

fn add_npm_packages_to_json(
  json: &mut serde_json::Value,
  npm_resolver: &NpmPackageResolver,
) {
  // ideally deno_graph could handle this, but for now we just modify the json here
  let snapshot = npm_resolver.snapshot();
  let json = json.as_object_mut().unwrap();
  let modules = json.get_mut("modules").and_then(|m| m.as_array_mut());
  if let Some(modules) = modules {
    if modules.len() == 1
      && modules[0].get("kind").and_then(|k| k.as_str()) == Some("external")
    {
      // If there is only one module and it's "external", then that means
      // someone provided an npm specifier as a cli argument. In this case,
      // we want to show which npm package the cli argument resolved to.
      let module = &mut modules[0];
      let maybe_package = module
        .get("specifier")
        .and_then(|k| k.as_str())
        .and_then(|specifier| NpmPackageReference::from_str(specifier).ok())
        .and_then(|package_ref| {
          snapshot
            .resolve_package_from_deno_module(&package_ref.req)
            .ok()
        });
      if let Some(pkg) = maybe_package {
        if let Some(module) = module.as_object_mut() {
          module
            .insert("npmPackage".to_string(), pkg.id.as_serialized().into());
          // change the "kind" to be "npm"
          module.insert("kind".to_string(), "npm".into());
        }
      }
    } else {
      // Filter out npm package references from the modules and instead
      // have them only listed as dependencies. This is done because various
      // npm specifiers modules in the graph are really just unresolved
      // references. So there could be listed multiple npm specifiers
      // that would resolve to a single npm package.
      for i in (0..modules.len()).rev() {
        if modules[i].get("kind").and_then(|k| k.as_str()) == Some("external") {
          modules.remove(i);
        }
      }
    }

    for module in modules.iter_mut() {
      let dependencies = module
        .get_mut("dependencies")
        .and_then(|d| d.as_array_mut());
      if let Some(dependencies) = dependencies {
        for dep in dependencies.iter_mut() {
          if let serde_json::Value::Object(dep) = dep {
            let specifier = dep.get("specifier").and_then(|s| s.as_str());
            if let Some(specifier) = specifier {
              if let Ok(npm_ref) = NpmPackageReference::from_str(specifier) {
                if let Ok(pkg) =
                  snapshot.resolve_package_from_deno_module(&npm_ref.req)
                {
                  dep.insert(
                    "npmPackage".to_string(),
                    pkg.id.as_serialized().into(),
                  );
                }
              }
            }
          }
        }
      }
    }
  }

  let mut sorted_packages = snapshot.all_packages();
  sorted_packages.sort_by(|a, b| a.id.cmp(&b.id));
  let mut json_packages = serde_json::Map::with_capacity(sorted_packages.len());
  for pkg in sorted_packages {
    let mut kv = serde_json::Map::new();
    kv.insert("name".to_string(), pkg.id.name.to_string().into());
    kv.insert("version".to_string(), pkg.id.version.to_string().into());
    let mut deps = pkg.dependencies.values().collect::<Vec<_>>();
    deps.sort();
    let deps = deps
      .into_iter()
      .map(|id| serde_json::Value::String(id.as_serialized()))
      .collect::<Vec<_>>();
    kv.insert("dependencies".to_string(), deps.into());

    json_packages.insert(pkg.id.as_serialized(), kv.into());
  }

  json.insert("npmPackages".to_string(), json_packages.into());
}

struct TreeNode {
  text: String,
  children: Vec<TreeNode>,
}

impl TreeNode {
  pub fn from_text(text: String) -> Self {
    Self {
      text,
      children: Default::default(),
    }
  }
}

fn print_tree_node<TWrite: Write>(
  tree_node: &TreeNode,
  writer: &mut TWrite,
) -> fmt::Result {
  fn print_children<TWrite: Write>(
    writer: &mut TWrite,
    prefix: &str,
    children: &Vec<TreeNode>,
  ) -> fmt::Result {
    const SIBLING_CONNECTOR: char = '├';
    const LAST_SIBLING_CONNECTOR: char = '└';
    const CHILD_DEPS_CONNECTOR: char = '┬';
    const CHILD_NO_DEPS_CONNECTOR: char = '─';
    const VERTICAL_CONNECTOR: char = '│';
    const EMPTY_CONNECTOR: char = ' ';

    let child_len = children.len();
    for (index, child) in children.iter().enumerate() {
      let is_last = index + 1 == child_len;
      let sibling_connector = if is_last {
        LAST_SIBLING_CONNECTOR
      } else {
        SIBLING_CONNECTOR
      };
      let child_connector = if child.children.is_empty() {
        CHILD_NO_DEPS_CONNECTOR
      } else {
        CHILD_DEPS_CONNECTOR
      };
      writeln!(
        writer,
        "{} {}",
        colors::gray(format!(
          "{}{}─{}",
          prefix, sibling_connector, child_connector
        )),
        child.text
      )?;
      let child_prefix = format!(
        "{}{}{}",
        prefix,
        if is_last {
          EMPTY_CONNECTOR
        } else {
          VERTICAL_CONNECTOR
        },
        EMPTY_CONNECTOR
      );
      print_children(writer, &child_prefix, &child.children)?;
    }

    Ok(())
  }

  writeln!(writer, "{}", tree_node.text)?;
  print_children(writer, "", &tree_node.children)?;
  Ok(())
}

/// Precached information about npm packages that are used in deno info.
#[derive(Default)]
struct NpmInfo {
  package_sizes: HashMap<NpmPackageId, u64>,
  resolved_reqs: HashMap<NpmPackageReq, NpmPackageId>,
  packages: HashMap<NpmPackageId, NpmResolutionPackage>,
  specifiers: HashMap<ModuleSpecifier, NpmPackageReq>,
}

impl NpmInfo {
  pub fn build<'a>(
    graph: &'a ModuleGraph,
    npm_resolver: &'a NpmPackageResolver,
    npm_snapshot: &'a NpmResolutionSnapshot,
  ) -> Self {
    let mut info = NpmInfo::default();
    if !npm_resolver.has_packages() {
      return info; // skip going over the specifiers if there's no npm packages
    }

    for (specifier, _) in graph.specifiers() {
      if let Ok(reference) = NpmPackageReference::from_specifier(specifier) {
        info
          .specifiers
          .insert(specifier.clone(), reference.req.clone());
        if let Ok(package) =
          npm_snapshot.resolve_package_from_deno_module(&reference.req)
        {
          info.resolved_reqs.insert(reference.req, package.id.clone());
          if !info.packages.contains_key(&package.id) {
            info.fill_package_info(package, npm_resolver, npm_snapshot);
          }
        }
      }
    }

    info
  }

  fn fill_package_info<'a>(
    &mut self,
    package: &NpmResolutionPackage,
    npm_resolver: &'a NpmPackageResolver,
    npm_snapshot: &'a NpmResolutionSnapshot,
  ) {
    self.packages.insert(package.id.clone(), package.clone());
    if let Ok(size) = npm_resolver.package_size(&package.id) {
      self.package_sizes.insert(package.id.clone(), size);
    }
    for id in package.dependencies.values() {
      if !self.packages.contains_key(id) {
        if let Some(package) = npm_snapshot.package_from_id(id) {
          self.fill_package_info(package, npm_resolver, npm_snapshot);
        }
      }
    }
  }

  pub fn package_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&NpmResolutionPackage> {
    self
      .specifiers
      .get(specifier)
      .and_then(|package_req| self.resolved_reqs.get(package_req))
      .and_then(|id| self.packages.get(id))
  }
}

struct GraphDisplayContext<'a> {
  graph: &'a ModuleGraph,
  npm_info: NpmInfo,
  seen: HashSet<String>,
}

impl<'a> GraphDisplayContext<'a> {
  pub fn write<TWrite: Write>(
    graph: &'a ModuleGraph,
    npm_resolver: &'a NpmPackageResolver,
    writer: &mut TWrite,
  ) -> fmt::Result {
    let npm_snapshot = npm_resolver.snapshot();
    let npm_info = NpmInfo::build(graph, npm_resolver, &npm_snapshot);
    Self {
      graph,
      npm_info,
      seen: Default::default(),
    }
    .into_writer(writer)
  }

  fn into_writer<TWrite: Write>(mut self, writer: &mut TWrite) -> fmt::Result {
    if self.graph.roots.is_empty() || self.graph.roots.len() > 1 {
      return writeln!(
        writer,
        "{} displaying graphs that have multiple roots is not supported.",
        colors::red("error:")
      );
    }

    let root_specifier = self.graph.resolve(&self.graph.roots[0].0);
    match self.graph.try_get(&root_specifier) {
      Ok(Some(root)) => {
        if let Some(cache_info) = root.maybe_cache_info.as_ref() {
          if let Some(local) = &cache_info.local {
            writeln!(
              writer,
              "{} {}",
              colors::bold("local:"),
              local.to_string_lossy()
            )?;
          }
          if let Some(emit) = &cache_info.emit {
            writeln!(
              writer,
              "{} {}",
              colors::bold("emit:"),
              emit.to_string_lossy()
            )?;
          }
          if let Some(map) = &cache_info.map {
            writeln!(
              writer,
              "{} {}",
              colors::bold("map:"),
              map.to_string_lossy()
            )?;
          }
        }
        writeln!(writer, "{} {}", colors::bold("type:"), root.media_type)?;
        let total_modules_size =
          self.graph.modules().map(|m| m.size() as f64).sum::<f64>();
        let total_npm_package_size = self
          .npm_info
          .package_sizes
          .values()
          .map(|s| *s as f64)
          .sum::<f64>();
        let total_size = total_modules_size + total_npm_package_size;
        let dep_count = self.graph.modules().count() - 1
          + self.npm_info.packages.len()
          - self.npm_info.resolved_reqs.len();
        writeln!(
          writer,
          "{} {} unique",
          colors::bold("dependencies:"),
          dep_count,
        )?;
        writeln!(
          writer,
          "{} {}",
          colors::bold("size:"),
          display::human_size(total_size),
        )?;
        writeln!(writer)?;
        let root_node = self.build_module_info(root, false);
        print_tree_node(&root_node, writer)?;
        Ok(())
      }
      Err(ModuleGraphError::Missing(_)) => {
        writeln!(
          writer,
          "{} module could not be found",
          colors::red("error:")
        )
      }
      Err(err) => {
        writeln!(writer, "{} {}", colors::red("error:"), err)
      }
      Ok(None) => {
        writeln!(
          writer,
          "{} an internal error occurred",
          colors::red("error:")
        )
      }
    }
  }

  fn build_dep_info(&mut self, dep: &Dependency) -> Vec<TreeNode> {
    let mut children = Vec::with_capacity(2);
    if !dep.maybe_code.is_none() {
      if let Some(child) = self.build_resolved_info(&dep.maybe_code, false) {
        children.push(child);
      }
    }
    if !dep.maybe_type.is_none() {
      if let Some(child) = self.build_resolved_info(&dep.maybe_type, true) {
        children.push(child);
      }
    }
    children
  }

  fn build_module_info(&mut self, module: &Module, type_dep: bool) -> TreeNode {
    enum PackageOrSpecifier {
      Package(NpmResolutionPackage),
      Specifier(ModuleSpecifier),
    }

    use PackageOrSpecifier::*;

    let package_or_specifier =
      match self.npm_info.package_from_specifier(&module.specifier) {
        Some(package) => Package(package.clone()),
        None => Specifier(module.specifier.clone()),
      };
    let was_seen = !self.seen.insert(match &package_or_specifier {
      Package(package) => package.id.as_serialized(),
      Specifier(specifier) => specifier.to_string(),
    });
    let header_text = if was_seen {
      let specifier_str = if type_dep {
        colors::italic_gray(&module.specifier).to_string()
      } else {
        colors::gray(&module.specifier).to_string()
      };
      format!("{} {}", specifier_str, colors::gray("*"))
    } else {
      let specifier_str = if type_dep {
        colors::italic(&module.specifier).to_string()
      } else {
        module.specifier.to_string()
      };
      let header_text = match &package_or_specifier {
        Package(package) => {
          format!("{} - {}", specifier_str, package.id.version)
        }
        Specifier(_) => specifier_str,
      };
      let maybe_size = match &package_or_specifier {
        Package(package) => {
          self.npm_info.package_sizes.get(&package.id).copied()
        }
        Specifier(_) => module
          .maybe_source
          .as_ref()
          .map(|s| s.as_bytes().len() as u64),
      };
      format!("{} {}", header_text, maybe_size_to_text(maybe_size))
    };

    let mut tree_node = TreeNode::from_text(header_text);

    if !was_seen {
      if let Some((_, type_dep)) = &module.maybe_types_dependency {
        if let Some(child) = self.build_resolved_info(type_dep, true) {
          tree_node.children.push(child);
        }
      }
      match &package_or_specifier {
        Package(package) => {
          tree_node.children.extend(self.build_npm_deps(package));
        }
        Specifier(_) => {
          for dep in module.dependencies.values() {
            tree_node.children.extend(self.build_dep_info(dep));
          }
        }
      }
    }
    tree_node
  }

  fn build_npm_deps(
    &mut self,
    package: &NpmResolutionPackage,
  ) -> Vec<TreeNode> {
    let mut deps = package.dependencies.values().collect::<Vec<_>>();
    deps.sort();
    let mut children = Vec::with_capacity(deps.len());
    for dep_id in deps.into_iter() {
      let maybe_size = self.npm_info.package_sizes.get(dep_id).cloned();
      let size_str = maybe_size_to_text(maybe_size);
      let mut child = TreeNode::from_text(format!(
        "npm:{} {}",
        dep_id.as_serialized(),
        size_str
      ));
      if let Some(package) = self.npm_info.packages.get(dep_id) {
        if !package.dependencies.is_empty() {
          let was_seen = !self.seen.insert(package.id.as_serialized());
          if was_seen {
            child.text = format!("{} {}", child.text, colors::gray("*"));
          } else {
            let package = package.clone();
            child.children.extend(self.build_npm_deps(&package));
          }
        }
      }
      children.push(child);
    }
    children
  }

  fn build_error_info(
    &mut self,
    err: &ModuleGraphError,
    specifier: &ModuleSpecifier,
  ) -> TreeNode {
    self.seen.insert(specifier.to_string());
    match err {
      ModuleGraphError::InvalidTypeAssertion { .. } => {
        self.build_error_msg(specifier, "(invalid import assertion)")
      }
      ModuleGraphError::LoadingErr(_, _) => {
        self.build_error_msg(specifier, "(loading error)")
      }
      ModuleGraphError::ParseErr(_, _) => {
        self.build_error_msg(specifier, "(parsing error)")
      }
      ModuleGraphError::ResolutionError(_) => {
        self.build_error_msg(specifier, "(resolution error)")
      }
      ModuleGraphError::UnsupportedImportAssertionType(_, _) => {
        self.build_error_msg(specifier, "(unsupported import assertion)")
      }
      ModuleGraphError::UnsupportedMediaType(_, _) => {
        self.build_error_msg(specifier, "(unsupported)")
      }
      ModuleGraphError::Missing(_) => {
        self.build_error_msg(specifier, "(missing)")
      }
    }
  }

  fn build_error_msg(
    &self,
    specifier: &ModuleSpecifier,
    error_msg: &str,
  ) -> TreeNode {
    TreeNode::from_text(format!(
      "{} {}",
      colors::red(specifier),
      colors::red_bold(error_msg)
    ))
  }

  fn build_resolved_info(
    &mut self,
    resolved: &Resolved,
    type_dep: bool,
  ) -> Option<TreeNode> {
    match resolved {
      Resolved::Ok { specifier, .. } => {
        let resolved_specifier = self.graph.resolve(specifier);
        Some(match self.graph.try_get(&resolved_specifier) {
          Ok(Some(module)) => self.build_module_info(module, type_dep),
          Err(err) => self.build_error_info(&err, &resolved_specifier),
          Ok(None) => TreeNode::from_text(format!(
            "{} {}",
            colors::red(specifier),
            colors::red_bold("(missing)")
          )),
        })
      }
      Resolved::Err(err) => Some(TreeNode::from_text(format!(
        "{} {}",
        colors::italic(err.to_string()),
        colors::red_bold("(resolve error)")
      ))),
      _ => None,
    }
  }
}

fn maybe_size_to_text(maybe_size: Option<u64>) -> String {
  colors::gray(format!(
    "({})",
    match maybe_size {
      Some(size) => display::human_size(size as f64),
      None => "unknown".to_string(),
    }
  ))
  .to_string()
}
